use futures::{future, stream};
use ::regex::Regex;
use std::sync::Arc;

use super::*;
use crate::component::{ComponentKind, Metadata, Provider as CProvider};
use crate::error::*;
use crate::event::*;
use crate::util::futures::{BoxFuture, BoxStream};
use crate::value::*;

pub const NAME: &'static str = "regex";

pub fn provider() -> Box<Provider> {
    Box::new(ProviderImpl)
}

struct ProviderImpl;

impl CProvider for ProviderImpl {
    fn metadata(&self) -> Metadata {
        Metadata {
            kind: ComponentKind::Filter,
            name: NAME,
        }
    }
}

impl Provider for ProviderImpl {
    fn new(&self, ctx: New) -> Result<Arc<Starter>> {
        Ok(Arc::new(StarterImpl {
            config: Config::parse(ctx.config)?,
        }))
    }
}

#[derive(Clone)]
struct Config {
    patterns: Vec<(String, Vec<Regex>)>,
}

impl Config {
    pub fn parse(mut value: Spanned<Value>) -> Result<Config> {
        let mut patterns = Vec::new();
        if let Some(patterns_v) = value.remove_opt("match")? {
            for (name, pats_v) in patterns_v.into_map()? {
                let mut pats = Vec::new();
                match pats_v.kind() {
                    ValueKind::String => {
                        pats.push(Self::parse_regex(pats_v)?);
                    }
                    ValueKind::List => {
                        for pat_v in pats_v.into_list()? {
                            pats.push(Self::parse_regex(pat_v)?);
                        }
                    }
                    _ => return Err(pats_v.new_error("expected String or List").into()),
                }
                patterns.push((name, pats));
            }
        }
        Ok(Config {
            patterns,
        })
    }

    fn parse_regex(s: Spanned<Value>) -> Result<Regex> {
        Regex::new(s.as_str()?)
            .map_err(move |_| s.new_error("invalid regular expression"))
    }
}

struct StarterImpl {
    config: Config,
}

impl Starter for StarterImpl {
    fn start(&self) -> BoxFuture<Box<Filter>, Error> {
        Box::new(future::ok(Box::new(FilterImpl {
            config: self.config.clone(),
        }) as Box<Filter>))
    }
}

struct FilterImpl {
    config: Config,
}

impl Filter for FilterImpl {
    fn filter(&mut self, mut event: Event) -> BoxStream<Event, Error> {
        let mut new_fields = Vec::new();
        for &(ref field, ref regexes) in &self.config.patterns {
            let value = event.fields().get(field).and_then(|v| v.as_string().ok());
            if let Some(value) = value {
                for regex in regexes {
                    if let Some(caps) = regex.captures_iter(value).next() {
                        for (i, name) in regex.capture_names().enumerate() {
                            if let Some(name) = name {
                                if let Some(cap_value) = caps.get(i) {
                                    new_fields.push((name.to_owned(), cap_value.as_str().to_owned()));
                                }
                            }
                        }
                    }
                }
            }
        }
        for (n, v) in new_fields {
            event.fields_mut().entry(n)
                .or_insert_with(|| Value::String(v));
        }
        Box::new(stream::once(Ok(event)))
    }
}