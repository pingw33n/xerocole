pub mod null;
pub mod stdout;

use futures::sink::BoxSink;
use std::sync::Arc;

use super::*;
use crate::error::Error;
use crate::event::*;
use crate::util::futures::BoxFuture;

#[derive(Default)]
pub struct CommonConfig {
    pub id: Option<String>,
}

pub struct New {
    pub config: Spanned<Value>,
    pub common_config: CommonConfig,
}

pub struct Started {
    pub sink: BoxSink<Event, Error>,
}

pub trait OutputProvider: Provider {
    fn new(&self, ctx: New) -> Result<Box<Output>>;
}

pub trait Output: 'static + Send + Sync {
    fn start(&self) -> BoxFuture<Started, Error>;
}