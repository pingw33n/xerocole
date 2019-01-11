pub mod debug;
pub mod plain;

use std::sync::Arc;

use super::*;
use error::*;
use event::*;

pub struct New {
    pub config: Spanned<Value>
}

pub trait CodecProvider: Provider {
    fn new(&self, ctx: New) -> Result<Arc<Codec>>;
}

pub trait Codec: 'static + Send + Sync {
    fn decode(&self, buf: &[u8]) -> Result<Vec<Event>>;
    fn encode_as_string(&self, event: &Event) -> Result<String>;

    fn encode_as_bytes(&self, event: &Event) -> Result<Vec<u8>> {
        self.encode_as_string(event)
            .map(|s| s.into_bytes())
    }
}