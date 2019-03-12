use std::sync::Arc;

use super::*;
use super::super::{frame, event};
use crate::error::*;
use crate::event::*;

pub fn factory(frame: Arc<frame::Factory>, event: Arc<event::Factory>) -> Arc<Factory> {
    Arc::new(FactoryImpl {
        frame,
        event,
    })
}

#[derive(Clone)]
struct FactoryImpl {
    frame: Arc<frame::Factory>,
    event: Arc<event::Factory>,
}

impl super::Factory for FactoryImpl {
    fn new(&self) -> Box<Decoder> {
        Box::new(DecoderImpl {
            frame: self.frame.new(),
            event: self.event.new(),
        })
    }
}

struct DecoderImpl {
    frame: Box<frame::Decoder>,
    event: Box<event::Decoder>,
}

impl DecoderImpl {
    pub fn new(frame: Box<frame::Decoder>, event: Box<event::Decoder>) -> Self {
        Self {
            frame,
            event,
        }
    }

    fn decode(&mut self, inp: &[u8], out: &mut Vec<Event>, finish: bool) -> Result<Decode> {
        let mut frames = Vec::new();
        let read = if finish {
            self.frame.finish(inp, &mut frames)?
        } else {
            self.frame.decode(inp, &mut frames)?
        }.read;
        let mut written = 0;
        for frame in frames {
            written += self.event.decode(&frame, out)?;
        }
        if finish {
            written += self.event.finish(out)?;
        }
        Ok(Decode {
            read,
            written,
        })
    }
}

impl Decoder for DecoderImpl {
    fn decode(&mut self, inp: &[u8], out: &mut Vec<Event>) -> Result<Decode> {
        self.decode(inp, out, false)
    }

    fn finish(&mut self, inp: &[u8], out: &mut Vec<Event>) -> Result<Decode> {
        self.decode(inp, out, true)
    }
}