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
use component::filter;
use component::input;
use component::output;
use metric::Metrics;
use tokio::timer::Interval;
use std::io::prelude::*;
use std::time::{Duration, Instant};

fn main() {
    env_logger::Builder::from_default_env()
        .format(|buf, record| {
            let thread = std::thread::current();
            if let Some(thread_name) = thread.name() {
                writeln!(buf, "[{} {:5} {}][{}] {}",
                    buf.precise_timestamp(), record.level(),
                    record.module_path().unwrap_or(""),
                    thread_name,
                    record.args())
            } else {
                writeln!(buf, "[{} {:5} {}][{:?}] {}",
                    buf.precise_timestamp(), record.level(),
                    record.module_path().unwrap_or(""),
                    thread.id(),
                    record.args())
            }
        })
        .init();

    let input = registry().input("file").unwrap().new(input::New {
        config: value!{{
            "path" => [
//                "/tmp/log.txt",
//                "/tmp/log.*.txt",
//                "misc/access.log".into(),
                "/tmp/log.txt.gz",
            ],
            "start_position" => "beginning",
        }}.into(),
        common_config: input::CommonConfig {
            .. Default::default()
        }})
        .unwrap();

    let metrics = Arc::new(Metrics::new());

    let mut ppl_builder = pipeline::PipelineBuilder::new(metrics.clone());
    ppl_builder
        .input("file".into(), None, input)
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
