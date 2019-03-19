pub mod event;
pub mod frame;
pub mod frame_event;
pub mod stream;

use std::cmp;

use crate::error::*;
use crate::event::*;

pub struct Buf {
    buf: Vec<u8>,
    read_pos: usize,
    write_pos: usize,
}

impl Buf {
    pub fn new() -> Self {
        Self {
            buf: Vec::new(),
            read_pos: 0,
            write_pos: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.buf.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn read_pos(&self) -> usize {
        self.read_pos
    }

    pub fn advance_read_pos(&mut self, amount: usize) {
        self.read_pos += amount;
        assert!(self.read_pos <= self.write_pos);
    }

    pub fn write_pos(&self) -> usize {
        self.write_pos
    }

    pub fn advance_write_pos(&mut self, amount: usize) {
        self.write_pos += amount;
        assert!(self.write_pos <= self.len());
    }

    pub fn resize(&mut self, new_len: usize) {
        self.buf.resize(new_len, 0);
        self.read_pos = cmp::min(self.read_pos, self.buf.len());
        self.write_pos = cmp::min(self.write_pos, self.buf.len());
    }

    pub fn grow(&mut self) {
        let new_len = cmp::max(self.buf.len(), 512) * 2;
        self.resize(new_len);
    }

    pub fn ensure_writeable(&mut self) {
        if self.write().is_empty() {
            if self.read_pos > 0 && self.len() / self.read_pos <= 2 {
                self.compact();
            }
            self.grow();
        }
    }

    pub fn read(&self) -> &[u8] {
        &self.buf[self.read_pos..self.write_pos]
    }

    pub fn write(&mut self) -> &mut [u8] {
        &mut self.buf[self.write_pos..]
    }

    pub fn compact(&mut self) {
        self.buf.drain(..self.read_pos);
        self.write_pos -= self.read_pos;
        self.read_pos = 0;
    }

    pub fn clear(&mut self) {
        self.buf.clear();
        self.read_pos = 0;
        self.write_pos = 0;
    }
}

struct Stream {
    decoder: Box<stream::Decoder>,
    buf: Buf,
}

pub struct BufDecoder {
    stream: Stream,
    frame_event: Box<frame_event::Decoder>,
    buf: Buf,
}

impl BufDecoder {
    pub fn new(stream: Box<stream::Decoder>, frame_event: Box<frame_event::Decoder>) -> Self {
        Self {
            stream: Stream {
                decoder: stream,
                buf: Buf::new(),
            },
            frame_event,
            buf: Buf::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.stream.buf.is_empty() &&
            self.buf.is_empty()
    }

    pub fn clear(&mut self) {
        self.stream.buf.clear();
        self.buf.clear();
    }

    pub fn writeable_buf(&mut self) -> &mut Buf {
        self.buf.ensure_writeable();
        &mut self.buf
    }

    pub fn decode(&mut self, out: &mut Vec<Event>) -> Result<usize> {
        self.decode0(out, false)
    }

    pub fn flush(&mut self, out: &mut Vec<Event>) -> Result<usize> {
        self.decode0(out, true)
    }

    fn decode0(&mut self, out: &mut Vec<Event>, flush: bool) -> Result<usize> {
        let mut written = 0;
        loop {
            let needs_more_input = if self.buf.read().len() > 0 {
                self.stream.buf.ensure_writeable();
                let r = self.stream.decoder.decode(self.buf.read(),
                    self.stream.buf.write())?;
                self.buf.advance_read_pos(r.read);
                self.stream.buf.advance_write_pos(r.written);
                r.needs_more_input()
            } else {
                true
            };

            let r = self.frame_event.decode(self.stream.buf.read(), out)?;
            self.stream.buf.advance_read_pos(r.read);
            written += r.written;
            if flush {
                let r = self.frame_event.flush(self.stream.buf.read(), out)?;
                self.stream.buf.advance_read_pos(r.read);
                written += r.written;
                break;
            }

            if r.written > 0 || needs_more_input {
                break;
            }
        }
        Ok(written)
    }
}