#![allow(dead_code)]
#![allow(proc_macro_derive_resolution_fallback)]
#![deny(non_snake_case)]

extern crate env_logger;
extern crate futures_mpmc;
extern crate futures_retry;
extern crate glob;
extern crate humantime;
#[macro_use] extern crate icecream;
#[macro_use] extern crate if_chain;
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
mod metric;
mod pipeline;
mod util;
mod value;

use futures::prelude::*;
use std::sync::Arc;

use component::*;
use component::codec;
use component::filter;
use component::input;
use component::output;
use metric::Metrics;
use tokio::timer::Interval;
use std::time::Instant;
use std::time::Duration;

fn main() {
    env_logger::init();

    let codec = registry().codec("plain").unwrap()
        .new(codec::New { config: value!{{ "charset" => "UTF-8" }}.into() }).unwrap();
    let input = registry().input("file").unwrap().new(input::New {
        config: value!{{
            "path" => [
                "/tmp/log.txt",
                "/tmp/log.*.txt",
//                "misc/access.log".into(),
            ],
            "start_position" => "beginning",
        }}.into(),
        common_config: input::CommonConfig {
            codec: Some(codec),
            .. Default::default()
        }})
        .unwrap();

    let metrics = Arc::new(Metrics::new());

    let mut ppl_builder = pipeline::PipelineBuilder::new(metrics.clone());
    ppl_builder
        .input(None, input)
        .filter(component::registry().filter("grok").unwrap().new(filter::New {
            config: value!{{
                "match" => {
                    "message" => r#"(?<controller>[^#]+)#(?<action>\w+)"#,
                }
            }}.into(),
            common_config: Default::default()
        }).unwrap())
        .output(component::registry().output("stdout").unwrap().new(output::New {
            config: value! {{}}.into(),
            common_config: Default::default()
        }).unwrap())
        .output(component::registry().output("null").unwrap().new(output::New {
            config: value! {{}}.into(),
            common_config: Default::default(),
        }).unwrap())
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
