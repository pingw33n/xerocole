use std::cmp;

use super::*;

pub struct Provider;

impl Provider {
    pub const NAME: &'static str = "plain";
}

impl crate::component::Provider for Provider {
    fn metadata(&self) -> Metadata {
        Metadata {
            kind: ComponentKind::StreamDecoder,
            name: Self::NAME,
        }
    }
}

impl DecoderProvider for Provider {
    fn new(&self, _ctx: New) -> Result<Arc<Factory>> {
        Ok(Arc::new(FactoryImpl {}))
    }
}

struct FactoryImpl {
}

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