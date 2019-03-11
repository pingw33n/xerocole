pub mod composite;

use std::sync::Arc;

use super::super::*;
use crate::error::*;
use crate::event::*;

#[derive(Default)]
pub struct New {
    pub config: Spanned<Value>,
}

pub trait DecoderProvider: Provider {
    fn new(&self, ctx: New) -> Result<Arc<Factory>>;
}

pub trait Factory: 'static + Send + Sync {
    fn new(&self) -> Box<Decoder>;
}

#[derive(Debug, Clone)]
pub struct Decode {
    /// Number of bytes read from `inp`.
    pub read: usize,

    /// Number of bytes written to `out`.
    pub written: usize,
}

pub trait Decoder: 'static + Send {
    fn decode(&mut self, inp: &[u8], out: &mut Vec<Event>) -> Result<Decode>;

    fn finish(&mut self, inp: &[u8], out: &mut Vec<Event>) -> Result<Decode>;
}
