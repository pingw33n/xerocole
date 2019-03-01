pub mod file;

use std::sync::Arc;

use super::*;
use crate::component::codec::Codec;
use crate::error::Error;
use crate::event::*;
use crate::util::futures::*;

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
    pub stream: BoxStream<Event, Error>,
    pub shutdown: signal::Sender,
}

pub trait InputProvider: Provider {
    fn new(&self, ctx: New) -> Result<Box<Input>>;
}

pub trait Input: Component {
    fn start(&self) -> BoxFuture<Started, Error>;
}