pub mod file;

use component::codec::Codec;
use super::Node;
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

pub trait Input: Node {
    fn start(self: Box<Self>) -> BoxFuture<Started, Error>;
}