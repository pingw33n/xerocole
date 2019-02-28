use futures::future;
use futures::prelude::*;

use super::*;
use super::super::*;
use crate::event::*;
use crate::util::futures::BoxFuture;

pub struct Provider;

impl Provider {
    pub const NAME: &'static str = "null";
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
        Ok(Box::new(NullOutput))
    }
}

struct NullOutput;

impl Output for NullOutput {
    fn start(&self) -> BoxFuture<Started, Error> {
        Box::new(future::ok(Started {
            sink: Box::new(NullSink),
        }))
    }
}

struct NullSink;

impl Sink for NullSink {
    type SinkItem = Event;
    type SinkError = Error;

    fn start_send(&mut self, _event: Self::SinkItem) -> StartSend<Self::SinkItem, Self::SinkError> {
        Ok(AsyncSink::Ready)
    }

    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        Ok(Async::Ready(()))
    }
}