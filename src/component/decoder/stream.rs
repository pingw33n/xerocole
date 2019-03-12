pub mod gzip;
pub mod plain;

use std::sync::Arc;

use crate::error::*;
use crate::value::*;

#[derive(Default)]
pub struct New {
    pub config: Spanned<Value>
}

pub trait Provider: 'static + super::super::Provider {
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

impl Decode {
    pub fn needs_more_input(&self) -> bool {
        self.read == 0 && self.written == 0
    }
}

pub trait Decoder: 'static + Send {
    fn decode(&mut self, inp: &[u8], out: &mut [u8]) -> Result<Decode>;
}