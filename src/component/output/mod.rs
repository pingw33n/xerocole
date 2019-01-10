pub mod null;
pub mod stdout;

use futures::sink::BoxSink;

use component::codec::Codec;
use error::Error;
use event::*;
use util::futures::BoxFuture;

#[derive(Default)]
pub struct CommonConfig {
    pub id: Option<String>,
    pub codec: Option<Box<Codec>>,
}

pub struct Started {
    pub sink: BoxSink<Event, Error>,
}

pub trait Output: Send + 'static {
    fn start(self: Box<Self>) -> BoxFuture<Started, Error>;
}