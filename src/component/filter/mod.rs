pub mod grok;

use std::sync::Arc;

use super::*;
use error::Error;
use event::*;
use util::futures::{BoxFuture, BoxStream};
use value::*;

#[derive(Default)]
pub struct CommonConfig {
    pub id: Option<String>,
}

pub struct New {
    pub config: Spanned<Value>,
    pub common_config: CommonConfig,
}

pub struct Started {
    pub instance: Arc<Instance>,
}

pub trait FilterProvider: Provider {
    fn new(&self, ctx: New) -> Result<Box<Filter>>;
}

pub trait Filter: 'static + Send {
    fn start(&self) -> BoxFuture<Started, Error>;
}

pub trait Instance: 'static + Send + Sync {
    fn filter(&self, event: Event) -> BoxStream<Event, Error>;
}