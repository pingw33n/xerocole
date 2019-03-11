pub mod text;

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

pub trait Decoder: 'static + Send {
    fn decode(&mut self, inp: &[u8], out: &mut Vec<Event>) -> Result<usize>;

    fn finish(&mut self, out: &mut Vec<Event>) -> Result<usize>;
}