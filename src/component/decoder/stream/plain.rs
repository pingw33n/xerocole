use std::cmp;

use super::*;
use crate::component::{ComponentKind, Metadata, Provider as CProvider};

pub const NAME: &'static str = "plain";

pub fn provider() -> Box<Provider> {
    Box::new(ProviderImpl)
}

struct ProviderImpl;

impl CProvider for ProviderImpl {
    fn metadata(&self) -> Metadata {
        Metadata {
            kind: ComponentKind::StreamDecoder,
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
    fn new(&self) -> Box<Decoder> {
        Box::new(DecoderImpl {})
    }
}

struct DecoderImpl {}

impl Decoder for DecoderImpl {
    fn decode(&mut self, inp: &[u8], out: &mut [u8]) -> Result<Decode> {
        let len = cmp::min(inp.len(), out.len());
        out[..len].copy_from_slice(&inp[..len]);
        Ok(Decode {
            read: len,
            written: len,
        })
    }
}