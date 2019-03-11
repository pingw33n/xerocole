use futures::future;
use futures::prelude::*;

use super::*;
use super::super::*;
use crate::event::*;
use crate::util::futures::*;

pub struct Provider;

impl Provider {
    pub const NAME: &'static str = "stdout";
}

impl super::super::Provider for Provider {
    fn metadata(&self) -> Metadata {
        Metadata {
            kind: ComponentKind::Output,
            name: Self::NAME,
        }
    }
}

impl OutputProvider for Provider {
    fn new(&self, _ctx: New) -> Result<Box<Output>> {
        let encoder_factory = registry().encoder("debug").unwrap().new(Default::default())?;
        Ok(Box::new(StdoutOutput {
            config: Config { encoder_factory },
        }))
    }
}

#[derive(Clone)]
struct Config {
    encoder_factory: Arc<encoder::Factory>,
}

#[derive(Clone)]
struct StdoutOutput {
    config: Config,
}

impl Output for StdoutOutput {
    fn start(&self) -> BoxFuture<Started, Error> {
        Box::new(future::ok(Started {
            sink: Box::new(StdoutSink {
                encoder: self.config.encoder_factory.new(),
            }),
        }))
    }
}

struct StdoutSink {
    encoder: Box<encoder::Encoder>,
}

impl Sink for StdoutSink {
    type SinkItem = Event;
    type SinkError = Error;

    fn start_send(&mut self, event: Self::SinkItem) -> StartSend<Self::SinkItem, Self::SinkError> {
        let mut buf = Vec::new();
        self.encoder.encode(&event, &mut buf)?;
        let s = String::from_utf8_lossy(&buf);

        Ok(match tokio_threadpool::blocking(move || println!("{}", s)).unwrap() {
            Async::Ready(()) => AsyncSink::Ready,
            Async::NotReady => AsyncSink::NotReady(event),
        })
    }

    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        Ok(Async::Ready(()))
    }
}