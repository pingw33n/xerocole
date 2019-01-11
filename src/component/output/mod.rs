pub mod null;
pub mod stdout;

use futures::sink::BoxSink;
use std::sync::Arc;

use super::*;
use component::codec::Codec;
use error::Error;
use event::*;
use util::futures::BoxFuture;

#[derive(Default)]
pub struct CommonConfig {
    pub id: Option<String>,
    pub codec: Option<Arc<Codec>>,
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

pub trait Output: Send + 'static {
    fn start(self: Box<Self>) -> BoxFuture<Started, Error>;
}