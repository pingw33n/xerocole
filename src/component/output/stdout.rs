use futures::future;
use futures::prelude::*;

use super::*;
use super::super::*;
use event::*;
use util::futures::BoxFuture;

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

struct Config {
    codec: Arc<Codec>,
}

struct StdoutOutput {
    config: Config,
}

impl Output for StdoutOutput {
    fn start(self: Box<Self>) -> BoxFuture<Started, Error> {
        Box::new(future::ok(Started {
            sink: Box::new(StdoutSink {
                config: self.config,
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
        // TODO asynchronously write to stdout
        println!("{}", s);
        Ok(AsyncSink::Ready)
    }

    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        Ok(Async::Ready(()))
    }
}