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
    fn new(&self, _config: Spanned<Value>, _common_config: CommonConfig) -> Result<Box<Output>> {
        Ok(Box::new(StdoutOutput))
    }
}

struct StdoutOutput;

impl Output for StdoutOutput {
    fn start(self: Box<Self>) -> BoxFuture<Started, Error> {
        Box::new(future::ok(Started {
            sink: Box::new(StdoutSink),
        }))
    }
}

struct StdoutSink;

impl Sink for StdoutSink {
    type SinkItem = Event;
    type SinkError = Error;

    fn start_send(&mut self, event: Self::SinkItem) -> StartSend<Self::SinkItem, Self::SinkError> {
        println!("{:#?}", event);
        Ok(AsyncSink::Ready)
    }

    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        Ok(Async::Ready(()))
    }
}