pub mod file;

use super::*;
use crate::error::Error;
use crate::event::*;
use crate::util::futures::*;

#[derive(Default)]
pub struct CommonConfig {
    pub id: Option<String>,
}

pub struct New {
    pub config: Spanned<Value>,
    pub common_config: CommonConfig,
}

pub struct Started {
    pub stream: BoxStream<Event, Error>,
    pub shutdown: signal::Sender,
}

pub trait Provider: 'static + super::Provider {
    fn new(&self, ctx: New) -> Result<Box<Input>>;
}

pub trait Input: Send {
    fn start(&self) -> BoxFuture<Started, Error>;
}