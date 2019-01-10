use futures::prelude::*;
use futures::stream::futures_unordered::FuturesUnordered;
use futures::sync::mpsc;
use futures_mpmc::{array as mpmc};
use std::sync::Arc;
use tokio::executor;

use component::input::Input;
use component::filter::Filter;
use component::output::Output;
use event::Event;
use metric::{self, Metrics};
use util::futures::*;

struct InputInfo {
    pub id: String,
    pub input: Box<Input>,
}

pub struct PipelineBuilder {
    concurrency: usize,
    in_queue_capacity: usize,
    out_queue_capacity: usize,
    inputs: Vec<InputInfo>,
    filters: Vec<Box<Filter>>,
    outputs: Vec<Box<Output>>,
    metrics: Arc<Metrics>,
}

impl PipelineBuilder {
    pub fn new(metrics: Arc<Metrics>) -> Self {
        Self {
            concurrency: num_cpus::get(),
            in_queue_capacity: 100,
            out_queue_capacity: 100,
            inputs: Vec::new(),
            filters: Vec::new(),
            outputs: Vec::new(),
            metrics,
        }
    }

    pub fn concurrency(&mut self, concurrency: usize) -> &mut Self {
        assert!(concurrency > 0);
        self.concurrency = concurrency;
        self
    }

    pub fn in_queue_capacity(&mut self, in_queue_capacity: usize) -> &mut Self {
        assert!(in_queue_capacity >= 2);
        self.in_queue_capacity = in_queue_capacity;
        self
    }

    pub fn out_queue_capacity(&mut self, out_queue_capacity: usize) -> &mut Self {
        assert!(out_queue_capacity >= 2);
        self.out_queue_capacity = out_queue_capacity;
        self
    }

    fn gen_input_id(&self, input: &Box<Input>) -> String {
        let name = input.provider_metadata().name;
        let mut i = 1;
        for input in &self.inputs {
            if input.input.provider_metadata().name == name {
                i += 1;
            }
        }
        format!("{}-{}", name, i)
    }

    pub fn input(&mut self, id: Option<String>, input: Box<Input>) -> &mut Self {
        let id = id.unwrap_or_else(|| self.gen_input_id(&input));
        self.inputs.push(InputInfo { id, input });
        self
    }

    pub fn filter(&mut self, filter: Box<Filter>) -> &mut Self {
        self.filters.push(filter);
        self
    }

    pub fn output(&mut self, output: Box<Output>) -> &mut Self {
        self.outputs.push(output);
        self
    }

    pub fn start(self) {
        let (in_queue_tx, in_queue_rx) = futures_mpmc::array::<Event>(self.in_queue_capacity);

        Self::start_inputs(self.inputs, in_queue_tx, self.metrics);

        let out_queue_tx = Self::start_outputs(self.outputs, self.out_queue_capacity);

        Self::start_filters(self.filters, in_queue_rx, out_queue_tx, self.concurrency);
    }

    fn start_inputs(inputs: Vec<InputInfo>, in_queue_tx: mpmc::Sender<Event>,
        metrics: Arc<Metrics>)
    {
        for input in inputs {
            let InputInfo { id, input } = input;
            let name = input.provider_metadata().name;
            let out_metric_name = format!("input.{}.out", id.clone());
            metrics.set(out_metric_name.clone(), metric::Value::Counter(metric::Number::Int(0)));
            executor::spawn(input.start()
                .inspect(move |_| info!("started input {} ({})", id, name))
                // TODO handle input start failures.
                .inspect_err(|e| error!("input start error: {:?}", e))
                .map(|i| i.stream)
                .flatten_stream()
                .map_err(|e| error!("input stream error: {:?}", e))
                .inspect(clone!(metrics => move |_| metrics.inc(&out_metric_name, 1)))
                .forward(in_queue_tx.clone()
                    .sink_map_err(|e| error!("in_queue_rx gone: {:?}", e)))
                .map(|_| {})
                .map_err(|e| error!("uncaught error: {:?}", e)));
        }
    }

    fn start_outputs(outputs: Vec<Box<Output>>, out_queue_capacity: usize) -> mpsc::Sender<Event> {
        let mut txs = Vec::new();

        for output in outputs {
            let (tx, rx) = mpsc::channel::<Event>(out_queue_capacity);
            executor::spawn(output.start()
                // TODO handle output start failures.
                .inspect_err(|e| error!("output start error: {:?}", e))
                .map(|o| o.sink)
                .and_then(move |output_sink| rx
                    .infallible()
                    // TODO don't fail the root future when output sink fails
                    .forward(output_sink)
                    .map(|_| {}))
                .map_err(|e| error!("output send error: {:?}", e))
            );
            txs.push(tx);
        }

        // Make broadcasting channel.
        let txs = Arc::new(txs);
        let (bcast_tx, bcast_rx) = mpsc::channel::<Event>(0);
        executor::spawn(bcast_rx
            .for_each(clone!(txs => move |event| {
                let mut futs = FuturesUnordered::new();
                for tx in &txs[..txs.len() - 1] {
                    futs.push(tx.clone().send(event.clone()))
                }
                futs.push(txs.last().unwrap().clone().send(event));

                futs.for_each(|_| Ok(()))
                    .map_err(|e| error!("error sending to one of output queues: {:?}", e))
            }))
        );

        bcast_tx
    }

    fn start_filters(filters: Vec<Box<Filter>>, in_queue_rx: mpmc::Receiver<Event>,
            out_queue_tx: mpsc::Sender<Event>, concurrency: usize) {
        for filter in filters {
            executor::spawn(filter.start()
                // TODO handle filter start failures.
                .map_err(|e| error!("filter start error: {:?}", e))
                .map(|s| s.instance)
                .and_then(clone!(in_queue_rx, out_queue_tx => move |f| {
                    for i in 0..concurrency {
                        executor::spawn(in_queue_rx.clone()
                            .infallible()
                            .and_then(clone!(f => move |e| f.filter(e)))
                            .flatten()
                            .map_err(|e| error!("filter error: {:?}", e))
                            .forward(out_queue_tx.clone()
                                .sink_map_err(|e| error!("error sending to out_queue: {:?}", e)))
                            .map(move |_| println!("filter task {} done", i))
                        );
                    }
                    Ok(())
                }))
            );
        }
    }
}