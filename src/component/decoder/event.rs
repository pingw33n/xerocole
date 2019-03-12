pub mod text;

use std::sync::Arc;

use crate::error::*;
use crate::event::*;
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

pub trait Decoder: 'static + Send {
    fn decode(&mut self, inp: &[u8], out: &mut Vec<Event>) -> Result<usize>;

    fn finish(&mut self, out: &mut Vec<Event>) -> Result<usize>;
}