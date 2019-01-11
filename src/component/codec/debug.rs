use super::*;
use super::super::*;
use event::*;

pub struct Provider;

impl Provider {
    pub const NAME: &'static str = "debug";
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
    fn new(&self, _ctx: New) -> Result<Arc<Codec>> {
        Ok(Arc::new(CodecImpl))
    }
}

struct CodecImpl;

impl Codec for CodecImpl {
    fn decode(&self, _buf: &[u8]) -> Result<Vec<Event>> {
        unimplemented!();
    }

    fn encode_as_string(&self, event: &Event) -> Result<String> {
        Ok(format!("{:#?}", event))
    }
}