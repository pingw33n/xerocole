pub mod debug;

use std::sync::Arc;

use super::*;
use crate::error::*;
use crate::event::*;

#[derive(Default)]
pub struct New {
    pub config: Spanned<Value>,
}

pub trait EncoderProvider: Provider {
    fn new(&self, ctx: New) -> Result<Arc<Factory>>;
}

pub trait Factory: 'static + Send + Sync {
    fn new(&self) -> Box<Encoder>;
}

pub trait Encoder: 'static + Send {
    fn encode(&mut self, event: &Event, out: &mut Vec<u8>) -> Result<()>;

    fn finish(&mut self, out: &mut Vec<u8>) -> Result<()>;
}