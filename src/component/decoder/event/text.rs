use super::*;
use crate::event::*;
use crate::value::*;

const CHARSET: &'static str = "charset";
const DEFAULT_CHARSET: &'static str = "UTF-8";
const DEFAULT_FIELD: &'static str = "message";

pub struct Provider;

impl Provider {
    pub const NAME: &'static str = "text";
}

impl crate::component::Provider for Provider {
    fn metadata(&self) -> Metadata {
        Metadata {
            kind: ComponentKind::EventDecoder,
            name: Self::NAME,
        }
    }
}

impl DecoderProvider for Provider {
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