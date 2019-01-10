pub mod file;

use super::*;
use component::codec::Codec;
use error::Error;
use event::*;
use util::futures::*;

#[derive(Default)]
pub struct CommonConfig {
    pub id: Option<String>,
    pub codec: Option<Box<Codec>>,
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

pub trait Input: Node {
    fn start(self: Box<Self>) -> BoxFuture<Started, Error>;
}