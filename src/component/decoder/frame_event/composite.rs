use std::sync::Arc;

use super::super::*;
use super::Decode;
use crate::error::*;
use crate::event::*;

#[derive(Clone)]
pub struct Factory {
    frame: Arc<frame::Factory>,
    event: Arc<event::Factory>,
}

impl Factory {
    pub fn new(frame: Arc<frame::Factory>, event: Arc<event::Factory>) -> Self {
        Self {
            frame,
            event,
        }
    }
}

impl super::Factory for Factory {
    fn new(&self) -> Box<super::Decoder> {
        Box::new(Decoder {
            frame: self.frame.new(),
            event: self.event.new(),
        })
    }
}


pub struct Decoder {
    frame: Box<frame::Decoder>,
    event: Box<event::Decoder>,
}

impl Decoder {
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

impl super::Decoder for Decoder {
    fn decode(&mut self, inp: &[u8], out: &mut Vec<Event>) -> Result<Decode> {
        self.decode(inp, out, false)
    }

    fn finish(&mut self, inp: &[u8], out: &mut Vec<Event>) -> Result<Decode> {
        self.decode(inp, out, true)
    }
}