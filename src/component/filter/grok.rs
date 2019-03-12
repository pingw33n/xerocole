use futures::{future, stream};
use onig::Regex;
use std::sync::Arc;

use super::*;
use crate::component::{ComponentKind, Metadata, Provider as CProvider};
use crate::error::*;
use crate::event::*;
use crate::util::futures::{BoxFuture, BoxStream};
use crate::value::*;

pub const NAME: &'static str = "grok";

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
    fn new(&self, ctx: New) -> Result<Box<Filter>> {
        Ok(Box::new(GrokFilter {
            config: Config::parse(ctx.config)?,
        }))
    }
}

struct CloneableRegex {
    regex: Regex,
    pattern: String,
}

impl CloneableRegex {
    pub fn new(pattern: impl Into<String>) -> ::std::result::Result<Self, onig::Error> {
        let pattern = pattern.into();
        Ok(Self {
            regex: Regex::new(&pattern)?,
            pattern,
        })
    }
}

impl Clone for CloneableRegex {
    fn clone(&self) -> Self {
        Self {
            regex: Regex::new(&self.pattern).unwrap(),
            pattern: self.pattern.clone(),
        }
    }
}

impl ::std::ops::Deref for CloneableRegex {
    type Target = Regex;

    fn deref(&self) -> &Self::Target {
        &self.regex
    }
}

#[derive(Clone)]
struct Config {
    patterns: Vec<(String, Vec<CloneableRegex>)>,
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

    fn parse_regex(s: Spanned<Value>) -> Result<CloneableRegex> {
        CloneableRegex::new(s.as_str()?)
            .map_err(move |_| s.new_error("invalid regular expression"))
    }
}

struct GrokFilter {
    config: Config,
}

impl Filter for GrokFilter {
    fn start(&self) -> BoxFuture<Started, Error> {
        Box::new(future::ok(Started {
            instance: Arc::new(GrokInstance {
                config: self.config.clone(),
            }),
        }))
    }
}

struct GrokInstance {
    config: Config,
}

impl Instance for GrokInstance {
    fn filter(&self, mut event: Event) -> BoxStream<Event, Error> {
        let mut new_fields = Vec::new();
        for &(ref field, ref regexes) in &self.config.patterns {
            let value = event.fields().get(field).and_then(|v| v.as_string().ok());
            if let Some(value) = value {
                for regex in regexes {
                    if let Some(cap) = regex.captures_iter(value).next() {
                        for (name, i) in regex.capture_names() {
                            let cap_value = cap.at(i[0] as usize);
                            if let Some(cap_value) = cap_value {
                                new_fields.push((name.to_owned(), cap_value.to_owned()));
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