use futures::prelude::*;
use futures::{future, stream};
use glob;
use libc;
use memchr;
use stream_cancel::{StreamExt as ScStreamExt};
use std::cmp;
use std::collections::HashMap;
use std::ffi::CString;
use std::fs::File;
use std::io::{self, Read};
use std::mem;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::executor;
use tokio::timer::Interval;

use super::*;
use super::super::*;
use error::*;
use event::*;
use util::futures::{*, stream::StreamExt};
use value::*;

pub struct Provider;

impl Provider {
    pub const NAME: &'static str = "file";

    fn with_config(config: Config, common_config: CommonConfig) -> Result<Box<Input>> {
        Ok(Box::new(FileInput {
            config,
            common_config,
        }))
    }
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
    fn new(&self, config: Spanned<Value>, common_config: CommonConfig) -> Result<Box<Input>> {
        Self::with_config(Config::parse(config)?, common_config)
    }
}

enum StartFrom {
    Beginning,
    End,
}

struct Config {
    path_patterns: Vec<String>,
    start_from: StartFrom,
}

impl Config {
    fn parse(mut value: Spanned<Value>) -> Result<Self> {
        let path_pattern_strs = value.remove("path")?.into_list()?;
        let mut path_patterns = Vec::new();
        for p in path_pattern_strs {
            path_patterns.push(p.into_string()?);
        }
        ic!(path_patterns);

        let start_from = if let Some(s) = value.remove_opt("start_position")? {
            match s.as_str()? {
                "beginning" => StartFrom::Beginning,
                "end" => StartFrom::End,
                _ => return Err(Error::ValueError(ValueError {
                    msg: "expected one of [\"beginning\", \"end\"]".into(),
                    span: s.span.clone(),
                })),
            }
        } else {
            StartFrom::Beginning
        };

        Ok(Self {
            path_patterns,
            start_from,
        })
    }
}

struct FileInput {
    config: Config,
    common_config: CommonConfig,
}

impl Node for FileInput {
    fn provider_metadata(&self) -> Metadata {
        use super::super::{Provider as P};
        Provider.metadata()
    }
}

impl Input for FileInput {
    fn start(self: Box<Self>) -> BoxFuture<Started, Error> {
        let state = Arc::new(Mutex::new(State::new()));

        let this = *self;
        let mut codec = this.common_config.codec.unwrap();
        let path_patterns = this.config.path_patterns;

        let (shutdown_tx, shutdown_rx) = signal::signal();
        let (trigger_tx, trigger_rx) = pulse::pulse();

        executor::spawn(Interval::new(Instant::now(), Duration::from_secs(5))
            .take_until(shutdown_rx.clone().map(|_| {}))
            .map_err(|e| error!("{}", e))
            .for_each(clone!(state, trigger_tx => move |_| {
                let mut discovered_files = Vec::new();
                for path_pattern in &path_patterns {
                    debug!("discovering files in {}", path_pattern);
                    for path in try_cont!(glob::glob(path_pattern).map_err(|e| error!("{}", e))) {
                        let path = try_cont!(path.map_err(|e| error!("{}", e)));
                        let id = try_cont!(file_id(&path)
                            .map_err(|e| error!("couldn't get file id: {}", e)));
                        discovered_files.push((path, id));
                    }
                }

                if !discovered_files.is_empty() {
                    let mut trigger = false;
                    let mut state = state.lock().unwrap();
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
                }

                Ok(())
            }))
        );

        let stream: BoxStream<Event, Error> = Box::new(
                Interval::new(Instant::now() + Duration::from_millis(1000), Duration::from_secs(5))
            .map(|_| {})
            .map_err(|_| Error::Generic("timer"))
            .select(trigger_rx.infallible())
            .take_until(shutdown_rx.clone())
            .and_then(clone!(state => move |_| {
                let mut state = state.lock().unwrap();

                if state.files.is_empty() {
                    return Ok(false);
                }

                state.cur_file_idx %= state.files.len();

                let i = state.cur_file_idx;
                let file = &mut state.files[i];

                if file.file.is_none() {
                    debug!("opening file: {:?}", file.path);
                    file.file = Some(File::open(&file.path)?);
                }

                file.len = file.file.as_ref().unwrap().metadata()?.len();

                Ok(true)
            }))
            .filter(|&v| v)
            .and_then(clone!(state, trigger_tx => move |_| {
                let mut state = state.lock().unwrap();
                let (events, done) = loop {
                    let i = state.cur_file_idx;
                    let file = &mut state.files[i];

                    if file.offset > file.len {
                        // TODO handle file shrunk.
                        file.offset = 0;
                    }
                    if file.offset < file.len {
                        const BUF_LEN: usize = 32768;
                        let len = file.buf.len();
                        let can_read = cmp::min(file.len - file.offset,
                            (BUF_LEN - len) as u64) as usize;
                        let end = len + can_read;
                        file.buf.resize(end, 0);
                        let read = match file.file.as_mut().unwrap().read(&mut file.buf[len..end]) {
                            Ok(v) => v,
                            Err(e) => {
                                error!("error reading file {}: {}",
                                    file.path.to_str().unwrap_or("?"), e);
                                return Err(e.into());
                            }
                        };
                        file.buf.truncate(len + read);
                        file.offset += read as u64;
                    } else {
                        break (Vec::new(), true);
                    }

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
                    break (events, false)
                };
                if done {
                    state.cur_file_idx += 1;
                }
                if !done || state.cur_file_idx < state.files.len() {
                    trigger_tx.signal();
                }
                Ok(stream::iter_ok(events))
            }))
            .map_err(clone!(state => move |e| {
                let mut state = state.lock().unwrap();
                state.cur_file_idx += 1;
                if state.cur_file_idx < state.files.len() {
                    trigger_tx.signal();
                }
                e
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