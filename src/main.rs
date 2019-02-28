#![allow(dead_code)]
#![deny(non_snake_case)]
#![deny(unused_imports)]
#![deny(unused_must_use)]

#[macro_use]
mod macros;

mod error;
mod event;
mod component;
mod metric;
mod pipeline;
mod retry;
mod util;
mod value;

use futures::prelude::*;
use log::*;
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
        .graph(pipeline::Node::Filters((
                vec![component::registry().filter("grok").unwrap().new(filter::New {
                        config: value!{{
                            "match" => {
                                "message" => r#"(?<controller>[^#]+)#(?<action>\w+)"#,
                            }
                        }}.into(),
                        common_config: Default::default()
                    }).unwrap()],
                Box::new(pipeline::Node::Outputs(vec![
                    component::registry().output("stdout").unwrap().new(output::New {
                        config: value! {{}}.into(),
                        common_config: Default::default()
                    }).unwrap(),
                    component::registry().output("null").unwrap().new(output::New {
                        config: value! {{}}.into(),
                        common_config: Default::default(),
                    }).unwrap(),
                ])))))
    ;

    let mut rt = tokio::runtime::Runtime::new().unwrap();

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
