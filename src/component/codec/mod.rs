pub mod plain;

use super::*;
use error::*;
use event::*;

pub trait CodecProvider: Provider {
    fn new(&self, config: Spanned<Value>) -> Result<Box<Codec>>;
}

pub trait Codec: Send + 'static {
    fn decode(&mut self, buf: &[u8]) -> Result<Vec<Event>>;
    fn encode_as_string(&mut self, event: &Event) -> Result<String>;

    fn encode_as_bytes(&mut self, event: &Event) -> Result<Vec<u8>> {
        self.encode_as_string(event)
            .map(|s| s.into_bytes())
    }
}