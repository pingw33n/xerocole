pub mod codec;
pub mod filter;
pub mod input;
pub mod output;

use std::collections::HashMap;

use codec::CodecProvider;
use error::*;
use filter::FilterProvider;
use input::InputProvider;
use output::OutputProvider;
use value::*;

pub trait Provider: Send + Sync {
    fn metadata(&self) -> Metadata;
}

pub trait Node: Send {
    fn provider_metadata(&self) -> Metadata;
}

#[derive(Clone, Debug)]
pub struct Metadata {
    pub name: &'static str,
    pub kind: ComponentKind,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ComponentKind {
    Codec,
    Filter,
    Input,
    Output,
}

enum TypedProvider {
    Codec(Box<CodecProvider>),
    Filter(Box<FilterProvider>),
    Input(Box<InputProvider>),
    Output(Box<OutputProvider>),
}

impl TypedProvider {
    pub fn as_codec(&self) -> Option<&Box<CodecProvider>> {
        if let TypedProvider::Codec(ref v) = self {
            Some(v)
        } else {
            None
        }
    }

    pub fn as_filter(&self) -> Option<&Box<FilterProvider>> {
        if let TypedProvider::Filter(ref v) = self {
            Some(v)
        } else {
            None
        }
    }

    pub fn as_input(&self) -> Option<&Box<InputProvider>> {
        if let TypedProvider::Input(ref v) = self {
            Some(v)
        } else {
            None
        }
    }

    pub fn as_output(&self) -> Option<&Box<OutputProvider>> {
        if let TypedProvider::Output(ref v) = self {
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

    pub fn filter<'a>(&'a self, name: &str) -> Option<&'a dyn FilterProvider> {
        self.components.get(&(ComponentKind::Filter, name.to_string()))
            .and_then(|v| v.as_filter())
            .map(|v| v.as_ref())
    }

    pub fn register_filter(&mut self, provider: impl 'static + FilterProvider) {
        self.components.insert((ComponentKind::Filter, provider.metadata().name.into()),
            TypedProvider::Filter(Box::new(provider)));
    }

    pub fn input<'a>(&'a self, name: &str) -> Option<&'a dyn InputProvider> {
        self.components.get(&(ComponentKind::Input, name.to_string()))
            .and_then(|v| v.as_input())
            .map(|v| v.as_ref())
    }

    pub fn register_input(&mut self, provider: impl 'static + InputProvider) {
        self.components.insert((ComponentKind::Input, provider.metadata().name.into()),
            TypedProvider::Input(Box::new(provider)));
    }

    pub fn codec<'a>(&'a self, name: &str) -> Option<&'a dyn CodecProvider> {
        self.components.get(&(ComponentKind::Codec, name.to_string()))
            .and_then(|v| v.as_codec())
            .map(|v| v.as_ref())
    }

    pub fn register_codec(&mut self, provider: impl 'static + CodecProvider) {
        self.components.insert((ComponentKind::Codec, provider.metadata().name.into()),
            TypedProvider::Codec(Box::new(provider)));
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

        r.register_filter(filter::grok::Provider);

        r.register_input(input::file::Provider);

        r.register_codec(codec::debug::Provider);
        r.register_codec(codec::plain::Provider);

        r.register_output(output::null::Provider);
        r.register_output(output::stdout::Provider);

        r
    };
}

pub fn registry() -> &'static Registry {
    &*REGISTRY
}