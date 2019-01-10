use super::*;
use super::super::*;
use event::*;
use value::*;

pub struct Provider;

impl Provider {
    pub const NAME: &'static str = "plain";
}

impl super::super::Provider for Provider {
    fn metadata(&self) -> Metadata {
        Metadata {
            kind: ComponentKind::Codec,
            name: Self::NAME,
        }
    }
}

impl CodecProvider for Provider {
    fn new(&self, ctx: New) -> Result<Box<Codec>> {
        let charset = ctx.config.get_opt_str("charset")?.unwrap_or("UTF-8");
        // FIXME
        assert!(charset.eq_ignore_ascii_case("UTF-8"));
        Ok(Box::new(PlainCodec))
    }
}

struct PlainCodec;

impl Codec for PlainCodec {
    fn decode(&mut self, buf: &[u8]) -> Result<Vec<Event>> {
        let mut event = Event::new();
        event.fields_mut().insert("message".into(),
            Value::String(String::from_utf8_lossy(buf).into()));

        Ok(vec![event])
    }

    fn encode_as_string(&mut self, _event: &Event) -> Result<String> {
        unimplemented!();
    }
}