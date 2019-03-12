use futures::future;
use futures::prelude::*;

use super::*;
use crate::component::{ComponentKind, Metadata, Provider as CProvider};
use crate::event::*;
use crate::util::futures::BoxFuture;

pub const NAME: &'static str = "null";

pub fn provider() -> Box<Provider> {
    Box::new(ProviderImpl)
}

struct ProviderImpl;

impl CProvider for ProviderImpl {
    fn metadata(&self) -> Metadata {
        Metadata {
            kind: ComponentKind::Output,
            name: NAME,
        }
    }
}

impl Provider for ProviderImpl {
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