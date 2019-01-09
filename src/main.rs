#![allow(dead_code)]
#![allow(proc_macro_derive_resolution_fallback)]
#![deny(non_snake_case)]

extern crate env_logger;
extern crate futures_mpmc;
extern crate glob;
#[macro_use] extern crate icecream;
#[macro_use] extern crate lazy_static;
extern crate libc;
#[macro_use] extern crate log;
extern crate memchr;
extern crate num_cpus;
extern crate futures;
extern crate onig;
extern crate stream_cancel;
extern crate tokio;
extern crate tokio_threadpool;

#[macro_use]
mod macros;

mod error;
mod event;
mod component;
mod metrics;
mod pipeline;
mod util;
mod value;

use futures::prelude::*;
use onig::Regex;
use std::sync::Arc;

use component::*;
use metrics::Metrics;
use tokio::timer::Interval;
use std::time::Instant;
use std::time::Duration;

fn main() {
    env_logger::init();

    let codec = registry().codec("plain").unwrap()
        .new(value!{{ "charset" => "UTF-8" }}.into()).unwrap();
    let input = registry()
        .input("file").unwrap()
        .new(value!{{
            "path" => [
                "/tmp/log.txt",
                "/tmp/log.*.txt",
//                "misc/access.log".into(),
            ],
            "start_position" => "beginning",
        }}.into(), input::CommonConfig {
            id: "file-input-1".into(),
            codec: Some(codec),
        })
        .unwrap();

    let metrics = Arc::new(Metrics::new());

    let mut ppl_builder = pipeline::PipelineBuilder::new(metrics.clone());
    ppl_builder
        .input(None, input)
        .filter(Box::new(component::filter::grok::GrokFilter {
            patterns: vec![
                ("message".into(), vec![Regex::new(r#"(?<controller>[^#]+)#(?<action>\w+)"#).unwrap()]),
            ],
        }))
        .output(component::registry().output("stdout").unwrap().new(
            value!{{}}.into(),
            component::output::CommonConfig { id: "stdout".into(),
                codec: Some(component::codec::plain::Provider.new(value!{{}}.into()).unwrap()) }).unwrap())
        .output(component::output::null::Provider.new(value!{{}}.into(),
            component::output::CommonConfig { id: "null".into(), codec: None }).unwrap())
    ;

    let tp_size = num_cpus::get();

    let mut tp_builder = tokio_threadpool::Builder::new();
    tp_builder.name_prefix("core-")
        .pool_size(tp_size);

    let mut rt_builder = tokio::runtime::Builder::new();
    rt_builder.threadpool_builder(tp_builder);

    let mut rt = rt_builder.build().unwrap();

    rt.spawn(Interval::new(Instant::now(), Duration::from_secs(5))
        .map_err(|e| error!("{}", e))
        .for_each(clone!(metrics => move |_| {
            println!("{:?}", metrics);
            Ok(())
        })));

    rt.spawn(futures::lazy(move || {
        ppl_builder.start();
        Ok(())
    }));

    rt.shutdown_on_idle().wait().unwrap();
}
