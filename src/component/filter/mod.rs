pub mod grok;

use std::sync::Arc;

use error::Error;
use event::*;
use util::futures::{BoxFuture, BoxStream};

#[derive(Default)]
pub struct CommonConfig {
    pub id: Option<String>,
}

pub struct Started {
    pub instance: Arc<Instance>,
}

pub trait Filter: Send + 'static {
    fn start(self: Box<Self>) -> BoxFuture<Started, Error>;
}

pub trait Instance: Send + Sync + 'static {
    fn filter(&self, event: Event) -> BoxFuture<BoxStream<Event, Error>, Error>;
}