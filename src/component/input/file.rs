use futures::prelude::*;
use futures::{future, stream};
use glob;
use log::*;
use memchr;
use parking_lot::Mutex;
use stream_cancel::{StreamExt as ScStreamExt};
use std::cmp;
use std::collections::HashMap;
use std::fs::{self, File};
`use std::io;
use std::mem;
use std::os::unix::fs::FileExt;
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
        let (shutdown_tx, shutdown_rx) = signal::signal();
        let (trigger_tx, trigger_rx) = pulse::pulse();

        let stateh = Arc::new(Mutex::new(State::new(trigger_tx)));

        let codec = self.config.codec.clone();
        let path_patterns = Arc::new(self.config.path_patterns.clone());

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
                            let stat = try_cont!(stat(&path)
                                .map_err(|e| error!("couldn't get file stat: {}", e)));
                            discovered_files.push((path, stat));
                        }
                    }
                    discovered_files
                })).map_err(|e| error!("{:?}", e))
            }))
            .for_each(clone!(stateh => move |discovered_files| {
                if discovered_files.is_empty() {
                    return Ok(());
                }

                let mut trigger = false;
                let mut state = stateh.lock();
                for (path, stat) in discovered_files {
                    let next_idx = state.files.len();
                    let idx = *state.file_id_to_idx.entry(stat.id).or_insert(next_idx);
                    if idx == state.files.len() {
                        debug!("discovered new file: {:?} {:?}", path, stat);
                        state.files.push(Arc::new(Mutex::new(WatchedFile {
                            id: stat.id,
                            path,
                            file: None,
                            offset: 0,
                            len: stat.len,
                            buf: Vec::new(),
                        })));
                        trigger = true;
                    } else {
                        trace!("file is already being watched: {:?} {:?}", path, stat);
                        state.files[idx].lock().update(&stat, Some(path));
                    }
                }
                if trigger {
                    state.trigger();
                }
                mem::drop(state);

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
            .map_err(|e| panic!("{:?}", e)).infallible()
            .select(trigger_rx.infallible())
            .take_until(shutdown_rx.clone())
            .and_then(clone!(stateh => move |_| {
                let i = {
                    let mut state = stateh.lock();

                    if state.files.is_empty() {
                        return future::ok(false).into_box();
                    }

                    state.cur_file_idx %= state.files.len();

                    state.cur_file_idx
                };

                blocking(clone!(stateh => move || {
                        let fileh = stateh.lock().files[i].clone();
                        let mut file = fileh.lock();
                        match stat(&file.path) {
                            Ok(stat) => {
                                file.update(&stat, None);
                                Ok(true)
                            }
                            Err(e) => {
                                error!("error getting file stat: {:?} {:?}", file.path, e);
                                Err(e.wrap_id(ErrorId::Io))
                            }
                        }
                    }))
                    .map_err(|e| panic!("{:?}", e))
                    .and_then(|r| r)
                    .into_box()
            }))
            .filter(|&v| v)
            .and_then(clone!(stateh => move |_| {
                let mut state = stateh.lock();

                let file_idx = state.cur_file_idx;
                let fileh = state.files[file_idx].clone();
                let mut file = fileh.lock();

                if file.offset > file.len {
                    trace!("[{:?}] file.offset > file.len: {} > {}",
                        file.path, file.offset, file.len);
                    // TODO handle file shrunk.
                    file.offset = 0;
                }
                if file.offset == file.len {
                    trace!("[{:?}] file.offset == file.len: {}", file.path, file.offset);
                    state.next_file();
                    return future::ok(stream::empty().into_box()).into_box();
                }

                trace!("[{:?}] file.offset < file.len: {} < {}",
                    file.path, file.offset, file.len);
                blocking(clone!(fileh => move || fileh.lock().fill_buf()))
                    .map_err(|e| panic!("{:?}", e)).infallible()
                    .and_then(clone!(stateh, fileh, codec => move |r| {
                        r?;

                        let events = fileh.lock().create_events(&*codec);

                        stateh.lock().trigger_if_more_files();
                        Ok(stream::iter_ok(events).into_box())
                    }))
                    .into_box()
            }))
            .then(clone!(stateh => move |r| {
                match r {
                    r @ Ok(_) => r,
                    Err(e) => {
                        let mut state = stateh.lock();
                        warn!("processing file {:?} failed: {:?}",
                            state.files[state.cur_file_idx].lock().path, e);
                        state.next_file();
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

impl WatchedFile {
    pub fn update(&mut self, stat: &FileStat, path: Option<PathBuf>) {
        if let Some(path) = path {
            if self.path != path {
                debug!("file renamed: {:?} -> {:?}", self.path, path);
                self.path = path;
            }
        }
        if self.len != stat.len {
            debug!("file len changed: {:?} {} -> {}", self.path, self.len, stat.len);
            self.len = stat.len;
        }
    }

    pub fn fill_buf(&mut self) -> Result<()> {
        const BUF_LEN: usize = 32768;

        if self.file.is_none() {
            debug!("opening file: {:?}", self.path);
            self.file = Some(File::open(&self.path).wrap_err_id(ErrorId::Io)?);
        }

        let len = self.buf.len();
        let can_read = cmp::min(self.len - self.offset, (BUF_LEN - len) as u64) as usize;
        let end = len + can_read;
        self.buf.resize(end, 0);
        let read = self.file.as_ref().unwrap().read_at(&mut self.buf[len..end], self.offset)
            .wrap_err_id(ErrorId::Io)?;

        self.buf.truncate(len + read);
        self.offset += read as u64;

        Ok(())
    }

    pub fn create_events(&mut self, codec: &Codec) -> Vec<Event> {
        let mut events = Vec::new();
        let consumed = {
            let mut left = &self.buf[..];
            loop {
                let i = if let Some(i) = memchr::memchr(b'\n', left) {
                    i
                } else {
                    break;
                };
                for mut event in codec.decode(&left[..i]).unwrap() {
                    event.fields_mut().insert("path".into(),
                        Value::String(self.path.to_string_lossy().into()));
                    events.push(event);
                }

                left = &left[i + 1..];
            }
            self.buf.len() - left.len()
        };
        self.buf.drain(..consumed);
        events
    }
}

struct State {
    files: Vec<Arc<Mutex<WatchedFile>>>,
    file_id_to_idx: HashMap<FileId, usize>,
    cur_file_idx: usize,
    trigger: pulse::Sender,
}

impl State {
    pub fn new(trigger: pulse::Sender) -> Self {
        Self {
            files: Vec::new(),
            file_id_to_idx: HashMap::new(),
            cur_file_idx: 0,
            trigger,
        }
    }

    pub fn trigger(&self) {
        self.trigger.signal();
    }

    pub fn trigger_if_more_files(&self) {
        if self.cur_file_idx < self.files.len() {
            self.trigger();
        }
    }

    pub fn next_file(&mut self) {
        if self.cur_file_idx < self.files.len() {
            self.cur_file_idx += 1;
            self.trigger_if_more_files();
        }
    }
}

#[derive(Debug)]
struct FileStat {
    id: FileId,
    len: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
struct FileId((u64, u64));

fn stat<P: AsRef<Path>>(path: P) -> io::Result<FileStat> {
    use std::os::unix::fs::MetadataExt;
    let meta = fs::metadata(path)?;
    Ok(FileStat {
        id: FileId((meta.dev(), meta.ino())),
        len: meta.len(),
    })
}