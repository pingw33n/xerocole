pub mod delimited;

use std::sync::Arc;

use crate::error::*;
use crate::value::*;

#[derive(Default)]
pub struct New {
    pub config: Spanned<Value>,
}

pub trait Provider: 'static + super::super::Provider {
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

    fn flush<'a>(&mut self, inp: &'a [u8], out: &mut Vec<&'a [u8]>) -> Result<Decode>;
}