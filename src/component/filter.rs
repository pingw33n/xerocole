pub mod grok;

use std::sync::Arc;

use super::*;
use crate::error::Error;
use crate::event::*;
use crate::util::futures::{BoxFuture, BoxStream};
use crate::value::*;

#[derive(Default)]
pub struct CommonConfig {
    pub id: Option<String>,
}

pub struct New {
    pub config: Spanned<Value>,
    pub common_config: CommonConfig,
}

pub trait Provider: 'static + super::Provider {
    fn new(&self, ctx: New) -> Result<Arc<Starter>>;
}

pub trait Starter: 'static + Send + Sync {
    fn start(&self) -> BoxFuture<Box<Filter>, Error>;
}

pub trait Filter: 'static + Send {
    fn filter(&mut self, event: Event) -> BoxStream<Event, Error>;
}