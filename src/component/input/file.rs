use futures::prelude::*;
use futures::{future, stream};
use glob;
use log::*;
use parking_lot::Mutex;
use stream_cancel::{StreamExt as ScStreamExt};
use std::cmp;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io;
use std::mem;
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::executor;
use tokio::timer::Interval;

use super::*;
use crate::component::{ComponentKind, Metadata, Provider as CProvider};
use crate::component::decoder::BufDecoder;
use crate::error::*;
use crate::event::*;
use crate::util::futures::{*, stream::StreamExt};
use crate::util::futures::future::blocking;
use crate::value::*;

pub const NAME: &'static str = "file";

pub fn provider() -> Box<Provider> {
    Box::new(ProviderImpl)
}

struct ProviderImpl;

impl CProvider for ProviderImpl {
    fn metadata(&self) -> Metadata {
        Metadata {
            kind: ComponentKind::Input,
            name: NAME,
        }
    }
}

impl Provider for ProviderImpl {
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
    stream_decoder: Arc<decoder::stream::Factory>,
    frame_event_decoder: Arc<decoder::frame_event::Factory>,
}

impl Config {
    fn parse(mut value: Spanned<Value>, _common: CommonConfig) -> Result<Self> {
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

        let stream_decoder = registry().stream_decoder("gzip").unwrap().new(Default::default())?;
        let frame_event_decoder = decoder::frame_event::composite::factory(
            registry().frame_decoder("delimited").unwrap().new(Default::default())?,
            registry().event_decoder("text").unwrap().new(Default::default())?,
        );

        Ok(Self {
            path_patterns,
            start_from,
            stream_decoder,
            frame_event_decoder,
        })
    }
}

struct FileInput {
    config: Config,
}

impl Input for FileInput {
    fn start(&self) -> BoxFuture<Started, Error> {
        let (shutdown_tx, shutdown_rx) = signal::signal();
        let (trigger_tx, trigger_rx) = pulse::pulse();

        let stateh = Arc::new(Mutex::new(State::new(trigger_tx)));

        let stream_decoder = self.config.stream_decoder.clone();
        let frame_event_decoder = self.config.frame_event_decoder.clone();
        let path_patterns = Arc::new(self.config.path_patterns.clone());
        let start_from = self.config.start_from;

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
                }))
            }))
            .for_each(clone!(stateh, stream_decoder, frame_event_decoder => move |discovered_files| {
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
                            offset: match start_from {
                                StartFrom::Beginning => 0,
                                StartFrom::End => stat.len,
                            },
                            len: stat.len,
                            decoder: BufDecoder::new(
                                stream_decoder.new(),
                                frame_event_decoder.new()),
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
                    .infallible()
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
                blocking(clone!(fileh => move || {
                        fileh.lock().fill_buf()?;
                        fileh.lock().decode()
                    }))
                    .infallible()
                    .and_then(clone!(stateh => move |events| {
                        let events = events?;

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

struct WatchedFile {
    id: FileId,
    path: PathBuf,
    file: Option<File>,
    offset: u64,
    len: u64,
    decoder: BufDecoder,
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
        if self.file.is_none() {
            debug!("opening file: {:?}", self.path);
            self.file = Some(File::open(&self.path).wrap_err_id(ErrorId::Io)?);
        }

        let buf = self.decoder.writeable_buf();
        let can_read = cmp::min(self.len - self.offset, buf.len() as u64) as usize;
        let read = self.file.as_ref().unwrap().read_at(&mut buf.write()[..can_read], self.offset)
            .wrap_err_id(ErrorId::Io)?;
        buf.advance_write_pos(read);
        self.offset += read as u64;

        Ok(())
    }

    pub fn decode(&mut self) -> Result<Vec<Event>> {
        let mut events = Vec::new();
        while self.decoder.decode(&mut events)? > 0 {
        }
        for event in &mut events {
            event.fields_mut().insert("path".into(),
                Value::String(self.path.to_string_lossy().into()));
        }
        Ok(events)
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