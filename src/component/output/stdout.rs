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
    fn new(&self, ctx: New) -> Result<Box<Output>> {
        let codec = if let Some(codec) = ctx.common_config.codec {
            codec
        } else {
            registry().codec("debug").unwrap().new(codec::New { config: value!{{}}.into() })?
        };
        Ok(Box::new(StdoutOutput {
            config: Config { codec },
        }))
    }
}

#[derive(Clone)]
struct Config {
    codec: Arc<Codec>,
}

#[derive(Clone)]
struct StdoutOutput {
    config: Config,
}

impl Output for StdoutOutput {
    fn start(&self) -> BoxFuture<Started, Error> {
        Box::new(future::ok(Started {
            sink: Box::new(StdoutSink {
                config: self.config.clone(),
            }),
        }))
    }
}

struct StdoutSink {
    config: Config,
}

impl Sink for StdoutSink {
    type SinkItem = Event;
    type SinkError = Error;

    fn start_send(&mut self, event: Self::SinkItem) -> StartSend<Self::SinkItem, Self::SinkError> {
        let s = self.config.codec.encode_as_string(&event)?;
        Ok(match tokio_threadpool::blocking(move || println!("{}", s)).unwrap() {
            Async::Ready(()) => AsyncSink::Ready,
            Async::NotReady => AsyncSink::NotReady(event),
        })
    }

    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        Ok(Async::Ready(()))
    }
}