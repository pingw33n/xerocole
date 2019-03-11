use std::io::Write;

use super::*;
use crate::event::*;

pub struct Provider;

impl Provider {
    pub const NAME: &'static str = "debug";
}

impl crate::component::Provider for Provider {
    fn metadata(&self) -> Metadata {
        Metadata {
            kind: ComponentKind::Encoder,
            name: Self::NAME,
        }
    }
}

impl EncoderProvider for Provider {
    fn new(&self, _ctx: New) -> Result<Arc<Factory>> {
        Ok(Arc::new(FactoryImpl {}))
    }
}

struct FactoryImpl {}

impl Factory for FactoryImpl {
    fn new(&self) -> Box<Encoder> {
        Box::new(EncoderImpl {
            first: true,
        })
    }
}

struct EncoderImpl {
    first: bool,
}

impl Encoder for EncoderImpl {
    fn encode(&mut self, event: &Event, out: &mut Vec<u8>) -> Result<()> {
        if !self.first {
            out.push(b'\n');
        } else {
            self.first = false;
        }
        write!(out, "{:#?}", event).unwrap();
        Ok(())
    }

    fn finish(&mut self, _out: &mut Vec<u8>) -> Result<()> {
        self.first = true;
        Ok(())
    }
}