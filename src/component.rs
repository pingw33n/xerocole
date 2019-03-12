pub mod decoder;
pub mod encoder;
pub mod filter;
pub mod input;
pub mod output;

use lazy_static::lazy_static;
use std::collections::HashMap;

use crate::error::*;
use crate::value::*;
use output::OutputProvider;

pub trait Provider: Send + Sync {
    fn metadata(&self) -> Metadata;
}

pub trait Component: Send {
    fn provider_metadata(&self) -> Metadata;
}

#[derive(Clone, Debug)]
pub struct Metadata {
    pub name: &'static str,
    pub kind: ComponentKind,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ComponentKind {
    Encoder,
    EventDecoder,
    FrameDecoder,
    Filter,
    Input,
    Output,
    StreamDecoder,
}

enum TypedProvider {
    Encoder(Box<encoder::Provider>),
    EventDecoder(Box<decoder::event::Provider>),
    FrameDecoder(Box<decoder::frame::Provider>),
    Filter(Box<filter::Provider>),
    Input(Box<input::Provider>),
    Output(Box<OutputProvider>),
    StreamDecoder(Box<decoder::stream::Provider>),
}

impl TypedProvider {
    pub fn as_encoder(&self) -> Option<&Box<encoder::Provider>> {
        if let TypedProvider::Encoder(v) = self {
            Some(v)
        } else {
            None
        }
    }

    pub fn as_event_decoder(&self) -> Option<&Box<decoder::event::Provider>> {
        if let TypedProvider::EventDecoder(v) = self {
            Some(v)
        } else {
            None
        }
    }

    pub fn as_frame_decoder(&self) -> Option<&Box<decoder::frame::Provider>> {
        if let TypedProvider::FrameDecoder(v) = self {
            Some(v)
        } else {
            None
        }
    }

    pub fn as_filter(&self) -> Option<&Box<filter::Provider>> {
        if let TypedProvider::Filter(v) = self {
            Some(v)
        } else {
            None
        }
    }

    pub fn as_input(&self) -> Option<&Box<input::Provider>> {
        if let TypedProvider::Input(v) = self {
            Some(v)
        } else {
            None
        }
    }

    pub fn as_output(&self) -> Option<&Box<OutputProvider>> {
        if let TypedProvider::Output(v) = self {
            Some(v)
        } else {
            None
        }
    }

    pub fn as_stream_decoder(&self) -> Option<&Box<decoder::stream::Provider>> {
        if let TypedProvider::StreamDecoder(v) = self {
            Some(v)
        } else {
            None
        }
    }
}

pub struct Registry {
    components: HashMap<(ComponentKind, String), TypedProvider>,
}

impl Registry {
    pub fn new() -> Self {
        Self {
            components: HashMap::new(),
        }
    }

    pub fn filter<'a>(&'a self, name: &str) -> Option<&'a dyn filter::Provider> {
        self.components.get(&(ComponentKind::Filter, name.to_string()))
            .and_then(|v| v.as_filter())
            .map(|v| v.as_ref())
    }

    pub fn register_filter(&mut self, provider: Box<filter::Provider>) {
        self.components.insert((ComponentKind::Filter, provider.metadata().name.into()),
            TypedProvider::Filter(provider));
    }

    pub fn input<'a>(&'a self, name: &str) -> Option<&'a dyn input::Provider> {
        self.components.get(&(ComponentKind::Input, name.to_string()))
            .and_then(|v| v.as_input())
            .map(|v| v.as_ref())
    }

    pub fn register_input(&mut self, provider: Box<input::Provider>) {
        self.components.insert((ComponentKind::Input, provider.metadata().name.into()),
            TypedProvider::Input(provider));
    }

    pub fn encoder<'a>(&'a self, name: &str) -> Option<&'a dyn encoder::Provider> {
        self.components.get(&(ComponentKind::Encoder, name.to_string()))
            .and_then(|v| v.as_encoder())
            .map(|v| v.as_ref())
    }

    pub fn register_encoder(&mut self, provider: Box<encoder::Provider>) {
        self.components.insert((ComponentKind::Encoder, provider.metadata().name.into()),
            TypedProvider::Encoder(provider));
    }

    pub fn stream_decoder<'a>(&'a self, name: &str) -> Option<&'a dyn decoder::stream::Provider> {
        self.components.get(&(ComponentKind::StreamDecoder, name.to_string()))
            .and_then(|v| v.as_stream_decoder())
            .map(|v| v.as_ref())
    }

    pub fn register_stream_decoder(&mut self, provider: Box<decoder::stream::Provider>) {
        self.components.insert((ComponentKind::StreamDecoder, provider.metadata().name.into()),
            TypedProvider::StreamDecoder(provider));
    }

    pub fn event_decoder<'a>(&'a self, name: &str) -> Option<&'a dyn decoder::event::Provider> {
        self.components.get(&(ComponentKind::EventDecoder, name.to_string()))
            .and_then(|v| v.as_event_decoder())
            .map(|v| v.as_ref())
    }

    pub fn register_event_decoder(&mut self, provider: Box<decoder::event::Provider>) {
        self.components.insert((ComponentKind::EventDecoder, provider.metadata().name.into()),
            TypedProvider::EventDecoder(provider));
    }

    pub fn frame_decoder<'a>(&'a self, name: &str) -> Option<&'a dyn decoder::frame::Provider> {
        self.components.get(&(ComponentKind::FrameDecoder, name.to_string()))
            .and_then(|v| v.as_frame_decoder())
            .map(|v| v.as_ref())
    }

    pub fn register_frame_decoder(&mut self, provider: Box<decoder::frame::Provider>) {
        self.components.insert((ComponentKind::FrameDecoder, provider.metadata().name.into()),
            TypedProvider::FrameDecoder(provider));
    }

    pub fn output<'a>(&'a self, name: &str) -> Option<&'a dyn OutputProvider> {
        self.components.get(&(ComponentKind::Output, name.to_string()))
            .and_then(|v| v.as_output())
            .map(|v| v.as_ref())
    }

    pub fn register_output(&mut self, provider: impl 'static + OutputProvider) {
        self.components.insert((ComponentKind::Output, provider.metadata().name.into()),
            TypedProvider::Output(Box::new(provider)));
    }
}

lazy_static! {
    static ref REGISTRY: Registry = {
        let mut r = Registry::new();

        r.register_encoder(encoder::debug::provider());

        r.register_event_decoder(decoder::event::text::provider());

        r.register_filter(filter::grok::provider());

        r.register_frame_decoder(decoder::frame::delimited::provider());

        r.register_input(input::file::provider());

        r.register_output(output::null::Provider);
        r.register_output(output::stdout::Provider);

        r.register_stream_decoder(decoder::stream::gzip::provider());
        r.register_stream_decoder(decoder::stream::plain::provider());

        r
    };
}

pub fn registry() -> &'static Registry {
    &*REGISTRY
}