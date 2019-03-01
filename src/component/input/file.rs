use futures::prelude::*;
use futures::{future, stream};
use glob;
use libc;
use log::*;
use memchr;
use parking_lot::Mutex;
use stream_cancel::{StreamExt as ScStreamExt};
use std::cmp;
use std::collections::HashMap;
use std::ffi::CString;
use std::fs::{self, File};
use std::io::{self, Read};
use std::mem;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::executor;
use tokio::timer::Interval;

use super::*;
use super::Metadata;
use super::super::*;
use crate::error::*;
use crate::event::*;
use crate::util::futures::{*, stream::StreamExt};
use crate::util::futures::future::blocking;
use crate::value::*;

pub struct Provider;

impl Provider {
    pub const NAME: &'static str = "file";
}

impl super::super::Provider for Provider {
    fn metadata(&self) -> Metadata {
        Metadata {
            kind: ComponentKind::Input,
            name: Self::NAME,
        }
    }
}

impl InputProvider for Provider {
    fn new(&self, ctx: New) -> Result<Box<Input>> {
        Ok(Box::new(FileInput {
            config: Config::parse(ctx.config, ctx.common_config)?,
        }))
    }
}

#[derive(Clone, Copy, Debug)]
enum StartFrom {
    Beginning,
    End,
}

#[derive(Clone)]
struct Config {
    path_patterns: Vec<String>,
    start_from: StartFrom,
    codec: Arc<Codec>,
}

impl Config {
    fn parse(mut value: Spanned<Value>, common: CommonConfig) -> Result<Self> {
        let path_pattern_strs = value.remove("path")?.into_list()?;
        let mut path_patterns = Vec::new();
        for p in path_pattern_strs {
            path_patterns.push(p.into_string()?);
        }
        dbg!(&path_patterns);

        let start_from = if let Some(s) = value.remove_opt("start_position")? {
            match s.as_str()? {
                "beginning" => StartFrom::Beginning,
                "end" => StartFrom::End,
                _ => return Err(ErrorDetails::new("expected one of [\"beginning\", \"end\"]", s.span.clone()))
                    .wrap_err_id(ErrorId::Parse),
            }
        } else {
            StartFrom::Beginning
        };

        Ok(Self {
            path_patterns,
            start_from,
            codec: common.codec.unwrap(),
        })
    }
}

struct FileInput {
    config: Config,
}

impl Component for FileInput {
    fn provider_metadata(&self) -> Metadata {
        use super::super::{Provider as P};
        Provider.metadata()
    }
}

impl Input for FileInput {
    fn start(&self) -> BoxFuture<Started, Error> {
        let state = Arc::new(Mutex::new(State::new()));

        let codec = self.config.codec.clone();
        let path_patterns = Arc::new(self.config.path_patterns.clone());

        let (shutdown_tx, shutdown_rx) = signal::signal();
        let (trigger_tx, trigger_rx) = pulse::pulse();

        executor::spawn(Interval::new(Instant::now(), Duration::from_secs(5))
            .take_until(shutdown_rx.clone().map(|_| {}))
            .map_err(|e| error!("{}", e))
            .and_then(clone!(path_patterns => move |_| {
                blocking(clone!(path_patterns => move || {
                    let mut discovered_files = Vec::new();
                    for path_pattern in path_patterns.iter() {
                        debug!("discovering files in {}", path_pattern);
                        for path in try_cont!(glob::glob(path_pattern).map_err(|e| error!("{}", e))) {
                            let path = try_cont!(path.map_err(|e| error!("{}", e)));
                            let id = try_cont!(file_id(&path)
                                .map_err(|e| error!("couldn't get file id: {}", e)));
                            discovered_files.push((path, id));
                        }
                    }
                    discovered_files
                })).map_err(|e| error!("{:?}", e))
            }))
            .for_each(clone!(state, trigger_tx => move |discovered_files| {
                if discovered_files.is_empty() {
                    return Ok(());
                }

                let mut trigger = false;
                let mut state = state.lock();
                for (path, id) in discovered_files {
                    let len = state.files.len();
                    let idx = *state.file_id_to_idx.entry(id).or_insert(len);
                    if idx == state.files.len() {
                        debug!("discovered new file: {:?} {:?}", path, id);
                        state.files.push(WatchedFile {
                            id,
                            path,
                            file: None,
                            offset: 0,
                            len: 0,
                            buf: Vec::new(),
                        });
                        trigger = true;
                    } else {
                        trace!("file is already being watched: {:?} {:?}", path, id);
                    }
                }
                mem::drop(state);
                if trigger {
                    trigger_tx.signal();
                }

                Ok(())
            }))
            .inspect_err(clone!(shutdown_tx => move |_| {
                error!("discovery task failed, shutting down the file input");
                shutdown_tx.signal()
            }))
        );

        let stream: BoxStream<Event, Error> = Box::new(
                Interval::new(Instant::now() + Duration::from_millis(1000), Duration::from_secs(5))
            .map(|_| {})
            .map_err(|e| e.wrap_id(ErrorId::Unknown))
            .select(trigger_rx.infallible())
            .take_until(shutdown_rx.clone())
            .and_then(clone!(state => move |_| {
                let i = {
                    let mut state = state.lock();

                    if state.files.is_empty() {
                        return future::ok(false).into_box();
                    }

                    state.cur_file_idx %= state.files.len();

                    state.cur_file_idx
                };

                blocking(clone!(state => move || {
                        let mut state = state.lock();
                        let file = &mut state.files[i];
                        match fs::metadata(&file.path) {
                            Ok(meta) => {
                                file.len = meta.len();
                                true
                            }
                            Err(e) => {
                                error!("error getting file metadata `{:?}`: {:?}", file.path, e);
                                false
                            }
                        }
                    }))
                    .map_err(|e| e.wrap_id(ErrorId::Unknown))
                    .into_box()
            }))
            .filter(|&v| v)
            .and_then(clone!(state, trigger_tx => move |_| {
                let state_ = state.clone();
                let mut state = state_.lock();

                let file_idx = state.cur_file_idx;
                let file = &mut state.files[file_idx];

                if file.offset > file.len {
                    trace!("[{:?}] file.offset > file.len: {} > {}",
                        file.path, file.offset, file.len);
                    // TODO handle file shrunk.
                    file.offset = 0;
                }
                if file.offset < file.len {
                    trace!("[{:?}] file.offset < file.len: {} < {}",
                        file.path, file.offset, file.len);
                    const BUF_LEN: usize = 32768;
                    let len = file.buf.len();
                    let can_read = cmp::min(file.len - file.offset,
                        (BUF_LEN - len) as u64) as usize;
                    let end = len + can_read;
                    file.buf.resize(end, 0);

                    blocking(clone!(state_ => move || {
                            let mut state = state_.lock();
                            let file = &mut state.files[file_idx];
                            if file.file.is_none() {
                                debug!("opening file: {:?}", file.path);
                                file.file = Some(File::open(&file.path)?);
                            }
                            file.file.as_mut().unwrap().read(&mut file.buf[len..end])
                        }))
                        .map_err(|e| e.wrap_id(ErrorId::Unknown))
                        .and_then(clone!(state_, codec, trigger_tx => move |read| {
                            let mut state = state_.lock();
                            let file = &mut state.files[file_idx];

                            let read = match read {
                                Ok(v) => v,
                                Err(e) => {
                                    error!("error reading file {:?}: {}", file.path, e);
                                    return Err(e.wrap_id(ErrorId::Io));
                                }
                            };

                            file.buf.truncate(len + read);
                            file.offset += read as u64;

                            let mut events = Vec::new();
                            let consumed = {
                                let mut left = &file.buf[..];
                                loop {
                                    let i = if let Some(i) = memchr::memchr(b'\n', left) {
                                        i
                                    } else {
                                        break;
                                    };
                                    for mut event in codec.decode(&left[..i]).unwrap() {
                                        event.fields_mut().insert("path".into(),
                                            Value::String(file.path.to_string_lossy().into()));
                                        events.push(event);
                                    }

                                    left = &left[i + 1..];
                                }
                                file.buf.len() - left.len()
                            };
                            file.buf.drain(..consumed);
                            if state.cur_file_idx < state.files.len() {
                                trigger_tx.signal();
                            }
                            Ok(stream::iter_ok(events).into_box())
                        }))
                        .into_box()
                } else {
                    state.cur_file_idx += 1;
                    if state.cur_file_idx < state.files.len() {
                        trigger_tx.signal();
                    }
                    future::ok(stream::empty().into_box()).into_box()
                }
            }))
            .then(clone!(state => move |r| {
                match r {
                    r @ Ok(_) => r,
                    Err(e) => {
                        let mut state = state.lock();
                        warn!("processing file {:?} failed: {:?}",
                            state.files[state.cur_file_idx].path, e);
                        state.cur_file_idx += 1;
                        if state.cur_file_idx < state.files.len() {
                            trigger_tx.signal();
                        }
                        Ok(stream::iter_ok(Vec::new()).into_box())
                    }
                }
            }))
            .flatten()
        );

        Box::new(future::ok(Started {
            stream,
            shutdown: shutdown_tx,
        }))
    }
}

#[derive(Debug)]
struct WatchedFile {
    id: FileId,
    path: PathBuf,
    file: Option<File>,
    offset: u64,
    len: u64,
    buf: Vec<u8>,
}

struct State {
    files: Vec<WatchedFile>,
    file_id_to_idx: HashMap<FileId, usize>,
    cur_file_idx: usize,
}

impl State {
    pub fn new() -> Self {
        Self {
            files: Vec::new(),
            file_id_to_idx: HashMap::new(),
            cur_file_idx: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
struct FileId((u64, u64));

fn file_id<P: AsRef<Path>>(path: P) -> io::Result<FileId> {
    use std::os::unix::ffi::OsStrExt;
    unsafe {
        let path = CString::new(path.as_ref().as_os_str().as_bytes())
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        let mut stat: libc::stat = mem::uninitialized();
        if libc::lstat(path.as_ptr(), &mut stat as *mut _) == 0 {
            Ok(FileId((stat.st_dev as u64, stat.st_ino)))
        } else {
            Err(io::Error::last_os_error())
        }
    }
}