pub mod delimited;

use std::sync::Arc;

use super::super::*;
use crate::error::*;

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

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Decode {
    /// Number of bytes read from `inp`.
    pub read: usize,

    /// Number of frames written to `out`.
    pub written: usize,
}

pub trait Decoder: 'static + Send {
    fn decode<'a>(&mut self, inp: &'a [u8], out: &mut Vec<&'a [u8]>) -> Result<Decode>;

    fn finish<'a>(&mut self, inp: &'a [u8], out: &mut Vec<&'a [u8]>) -> Result<Decode>;
}