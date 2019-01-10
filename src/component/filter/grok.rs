use futures::{future, stream};
use onig::Regex;
use std::sync::Arc;

use super::*;
use super::super::*;
use error::Error;
use event::*;
use util::futures::{BoxFuture, BoxStream};
use value::*;

pub struct Provider;

impl Provider {
    pub const NAME: &'static str = "grok";
}

impl super::super::Provider for Provider {
    fn metadata(&self) -> Metadata {
        Metadata {
            kind: ComponentKind::Filter,
            name: Self::NAME,
        }
    }
}

impl FilterProvider for Provider {
    fn new(&self, ctx: New) -> Result<Box<Filter>> {
        Ok(Box::new(GrokFilter {
            config: Config::parse(ctx.config)?,
        }))
    }
}

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
        Regex::new(s.as_str()?).map_err(move |_| ValueError {
            msg: "invalid regular expression".into(),
            span: s.span,
        }.into())
    }
}

struct GrokFilter {
    config: Config,
}

impl Filter for GrokFilter {
    fn start(self: Box<Self>) -> BoxFuture<Started, Error> {
        Box::new(future::ok(Started {
            instance: Arc::new(GrokInstance {
                patterns: self.config.patterns,
            }),
        }))
    }
}

struct GrokInstance {
    patterns: Vec<(String, Vec<Regex>)>,
}

impl Instance for GrokInstance {
    fn filter(&self, mut event: Event) -> BoxFuture<BoxStream<Event, Error>, Error> {
        let mut new_fields = Vec::new();
        for &(ref field, ref regexes) in &self.patterns {
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
        Box::new(future::ok(Box::new(stream::once(Ok(event))) as BoxStream<_, _>))
    }
}