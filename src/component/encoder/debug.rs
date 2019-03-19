use std::io::Write;

use super::*;
use crate::component::{ComponentKind, Metadata, Provider as CProvider};
use crate::event::*;

pub const NAME: &'static str = "debug";

pub fn provider() -> Box<Provider> {
    Box::new(ProviderImpl)
}

struct ProviderImpl;

impl CProvider for ProviderImpl {
    fn metadata(&self) -> Metadata {
        Metadata {
            kind: ComponentKind::Encoder,
            name: NAME,
        }
    }
}

impl Provider for ProviderImpl {
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

    fn flush(&mut self, _out: &mut Vec<u8>) -> Result<()> {
        self.first = true;
        Ok(())
    }
}