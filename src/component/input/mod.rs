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

pub struct Started {
    pub stream: BoxStream<Event, Error>,
    pub shutdown: signal::Sender,
}

pub trait InputProvider: Provider {
    fn new(&self, config: Spanned<Value>, common_config: input::CommonConfig) -> Result<Box<Input>>;
}

pub trait Input: Node {
    fn start(self: Box<Self>) -> BoxFuture<Started, Error>;
}