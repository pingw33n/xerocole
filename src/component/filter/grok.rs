use futures::{future, stream};
use onig::Regex;
use std::sync::Arc;

use super::{Filter, Instance, Started};
use error::Error;
use event::*;
use util::futures::{BoxFuture, BoxStream};
use value::*;

pub struct GrokFilter {
    pub patterns: Vec<(String, Vec<Regex>)>,
}

impl Filter for GrokFilter {
    fn start(self: Box<Self>) -> BoxFuture<Started, Error> {
        Box::new(future::ok(Started {
            instance: Arc::new(GrokInstance {
                patterns: self.patterns,
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