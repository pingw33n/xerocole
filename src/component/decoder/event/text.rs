use super::*;
use crate::component::{ComponentKind, Metadata, Provider as CProvider};
use crate::event::*;
use crate::value::*;

pub const NAME: &'static str = "text";

const CHARSET: &'static str = "charset";
const DEFAULT_CHARSET: &'static str = "UTF-8";
const DEFAULT_FIELD: &'static str = "message";

pub fn provider() -> Box<Provider> {
    Box::new(ProviderImpl)
}

struct ProviderImpl;

impl CProvider for ProviderImpl {
    fn metadata(&self) -> Metadata {
        Metadata {
            kind: ComponentKind::EventDecoder,
            name: NAME,
        }
    }
}

impl Provider for ProviderImpl {
    fn new(&self, ctx: New) -> Result<Arc<Factory>> {
        let charset = ctx.config.get_opt_str(CHARSET)?.unwrap_or(DEFAULT_CHARSET);
        if charset != DEFAULT_CHARSET {
            unimplemented!();
        }

        Ok(Arc::new(FactoryImpl {
        }))
    }
}

struct FactoryImpl {}

impl Factory for FactoryImpl {
    fn new(&self) -> Box<Decoder> {
        Box::new(DecoderImpl {})
    }
}

struct DecoderImpl {
}

impl Decoder for DecoderImpl {
    fn decode(&mut self, inp: &[u8], out: &mut Vec<Event>) -> Result<usize> {
        let mut event = Event::new();
        event.fields_mut().insert(DEFAULT_FIELD.into(),
            Value::String(String::from_utf8_lossy(inp).into()));
        out.push(event);
        Ok(1)
    }

    fn finish(&mut self, _out: &mut Vec<Event>) -> Result<usize> {
        Ok(0)
    }
}