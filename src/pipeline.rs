use futures::future;
use futures::prelude::*;
use futures::stream::futures_unordered::FuturesUnordered;
use futures::sync::mpsc;
use futures_mpmc::{array as mpmc};
use futures_retry::{FutureRetry, StreamRetryExt};
use log::*;
use std::sync::Arc;
use std::time::Duration;
use tokio::executor;

use crate::component::input::Input;
use crate::component::filter;
use crate::component::output::Output;
use crate::error::*;
use crate::event::Event;
use crate::metric::{self, Metrics};
use crate::retry::RetryErrorHandler;
use crate::util::futures::*;

struct InputInfo {
    pub name: String,
    pub id: String,
    pub input: Box<Input>,
}

pub trait Predicate: 'static + Send + Sync {
    fn test(&self, event: &Event) -> Result<bool>;
}

pub enum Node {
    Filters((Vec<Arc<filter::Starter>>, Box<Node>)),
    Switch(Vec<(Arc<Predicate>, Box<Node>)>),
    Outputs(Vec<Box<Output>>),
}

enum IntNode {
    Filters {
        /// Index pointers to filters inside `StartGraph.filters`.
        /// At most one pointer can exist to a single filter.
        filters: Vec<usize>,
        next: Box<IntNode>
    },
    Switch(Vec<(Arc<Predicate>, Box<IntNode>)>),
    OutputGroup(usize),
}

impl IntNode {
    fn from(node: Node, filters: &mut Vec<Arc<filter::Starter>>,
        output_groups: &mut Vec<Vec<Box<Output>>>) -> Self
    {
        match node {
            Node::Filters((f, next)) => {
                let mut ids = Vec::new();
                for f in f {
                    ids.push(filters.len());
                    filters.push(f);
                }
                IntNode::Filters {
                    filters: ids,
                    next: Box::new(Self::from(*next, filters, output_groups)),
                }
            }
            Node::Switch(v) => IntNode::Switch(v.into_iter()
                .map(|(p, n)| (p, Box::new(Self::from(*n, filters, output_groups))))
                .collect()),
            Node::Outputs(o) => {
                let id = output_groups.len();
                output_groups.push(o);
                IntNode::OutputGroup(id)
            }
        }
    }
}

type FilterChain = Box<FnMut(Event) -> BoxStream<Event, Error> + Send>;

struct StartGraph<'a> {
    filters: &'a mut [Option<Box<filter::Filter>>],
    output_groups: &'a [mpsc::Sender<Event>],
    filter_concurrency: usize,
}

pub struct PipelineBuilder {
    in_queue_capacity: usize,
    filter_concurrency: usize,
    inputs: Vec<InputInfo>,
    graph: Option<Node>,
    metrics: Arc<Metrics>,
}

impl PipelineBuilder {
    pub fn new(metrics: Arc<Metrics>) -> Self {
        Self {
            filter_concurrency: num_cpus::get(),
            in_queue_capacity: 100,
            inputs: Vec::new(),
            graph: None,
            metrics,
        }
    }

    /// Input queue capacity. Input queue is the central queue where events from all inputs are
    /// placed. One ore more filter graphs are receiving from the input queue.
    pub fn in_queue_capacity(&mut self, in_queue_capacity: usize) -> &mut Self {
        assert!(in_queue_capacity >= 2);
        self.in_queue_capacity = in_queue_capacity;
        self
    }

    /// Number of concurrent filter graphs that are receiving from the input queue.
    pub fn filter_concurrency(&mut self, filter_concurrency: usize) -> &mut Self {
        assert!(filter_concurrency > 0);
        self.filter_concurrency = filter_concurrency;
        self
    }

    fn gen_input_id(&self, name: &str) -> String {
        let mut i = 1;
        for input in &self.inputs {
            if input.name == name {
                i += 1;
            }
        }
        format!("{}-{}", name, i)
    }

    pub fn input(&mut self, name: String, id: Option<String>, input: Box<Input>) -> &mut Self {
        let id = id.unwrap_or_else(|| self.gen_input_id(&name));
        self.inputs.push(InputInfo { name, id, input });
        self
    }

    pub fn graph(&mut self, graph: Node) -> &mut Self {
        self.graph = Some(graph);
        self
    }

    pub fn start(self) {
        assert!(self.graph.is_some());

        let (in_queue_tx, in_queue_rx) = mpmc::array::<Event>(self.in_queue_capacity);

        Self::start_inputs(self.inputs, in_queue_tx, self.metrics);

        let mut filters = Vec::new();
        let mut output_groups = Vec::new();
        let graph = Arc::new(IntNode::from(self.graph.unwrap(), &mut filters, &mut output_groups));

        let output_groups = Self::start_output_groups(output_groups, self.filter_concurrency,
            self.filter_concurrency);

        let filter_concurrency = self.filter_concurrency;

        for _ in 0..filter_concurrency {
            executor::spawn(Self::start_filters(&filters)
                .map_err(|e| error!("{:?}", e))
                .map(clone!(graph, in_queue_rx, output_groups => move |filters| {
                    let mut filters: Vec<_> = filters.into_iter().map(Some).collect();
                    Self::start_graph(&graph, Box::new(in_queue_rx.infallible()),
                        &mut StartGraph {
                            filters: &mut filters,
                            output_groups: &output_groups,
                            filter_concurrency,
                        });
                })));
        }
    }

    fn start_inputs(inputs: Vec<InputInfo>, in_queue_tx: mpmc::Sender<Event>,
        metrics: Arc<Metrics>)
    {
        for input in inputs {
            let InputInfo { id, name, input } = input;
            let out_metric_name = format!("input.{}.out", id.clone());
            metrics.set(out_metric_name.clone(), metric::Value::Counter(0.into()));

            let started = FutureRetry::new(clone!(id => move || {
                    info!("[{}] starting input", id);
                    input.start()
                }),
                RetryErrorHandler::new(None, Duration::from_secs(1), Duration::from_secs(60),
                    id.clone(), "starting input"));

            executor::spawn(started
                .inspect(clone!(id, name => move |_| info!("started input {} ({})", id, name)))
                // TODO handle input start failures.
                .map_err(|e| error!("input start error: {:?}", e))
                .map(clone!(id => move |i| i.stream
                    .retry(RetryErrorHandler::new(None, Duration::from_secs(1), Duration::from_secs(60),
                        id, "fetching input event"))
                    .map_err(|e| error!("input stream error: {:?}", e))))
                .flatten_stream()
                .inspect(clone!(metrics => move |_| metrics.inc(&out_metric_name, 1)))
                .forward(in_queue_tx.clone()
                    .sink_map_err(|e| error!("in_queue_rx gone: {:?}", e)))
                .map(clone!(id, name => move |_| info!("finished input {} ({})", id, name))));
        }
    }

    fn start_graph(node: &IntNode, stream: BoxStream<Event, Error>, ctx: &mut StartGraph)
    {
        match node {
            IntNode::Filters { filters: ids, next } => {
                let mut chain = Self::chain_filters(ids.iter()
                    .map(|&id| ctx.filters[id].take().unwrap()));
                let stream = Box::new(stream
                    .map(move |event| chain(event))
                    .flatten());
                Self::start_graph(next, stream, ctx);
            }
            IntNode::Switch(branches) => {
                let branches = branches.iter()
                    .map(|(p, n)| {
                        let (tx, rx) = mpsc::channel::<Event>(ctx.filter_concurrency);
                        let rx = Box::new(rx.infallible());
                        let tx = tx.sink_map_err(|e| error!("error sending to branch tx: {:?}", e));
                        Self::start_graph(n, rx, ctx);
                        (p.clone(), tx)
                    })
                    .collect::<Vec<_>>();
                executor::spawn(stream
                    .map_err(|_| {})
                    .for_each(move |event| {
                        for (pred, tx) in &branches {
                            match pred.test(&event) {
                                Ok(matched) => if matched {
                                    return Box::new(tx.clone().send(event)
                                        .map(|_| {})) as BoxFuture<_, _>;
                                }
                                Err(e) => {
                                    error!("branch predicate error: {:?}", e);
                                    break;
                                }
                            }
                        }
                        return Box::new(future::ok(()));
                    })
                    .map_err(|_| {})
                );
            }
            IntNode::OutputGroup(og) => {
                let tx = ctx.output_groups[*og].clone();
                executor::spawn(stream
                    .map_err(|_| {})
                    .forward(tx.clone()
                        .sink_map_err(|e| error!("error sending to output: {:?}", e)))
                    .map(move |_| debug!("filter task done")));
            }
        }
    }

    fn start_filters(starters: &[Arc<filter::Starter>])
            -> impl Future<Item=Vec<Box<filter::Filter + 'static>>, Error=Error> {
        let futs = starters.iter()
            .map(|s| {
                info!("starting filter");
                s.start()
            })
            .collect::<Vec<_>>();
        future::join_all(futs)
    }

    fn chain_filters(filters: impl IntoIterator<Item=Box<filter::Filter>>) -> FilterChain {
        let mut filters = filters.into_iter().collect::<Vec<_>>();
        assert!(!filters.is_empty());
        Box::new(move |event| {
            let mut r = filters[0].filter(event);
            for mut filter in filters.drain(1..) {
                // TODO implement proper FilterChain with less allocations
                r = Box::new(r
                    .map(move |event| filter.filter(event))
                    .flatten());
            }
            r
        })
    }

    fn start_output_groups(output_groups: Vec<Vec<Box<Output>>>, group_queue_capacity: usize,
        output_queue_capacity: usize) -> Vec<mpsc::Sender<Event>>
    {
        output_groups.into_iter()
            .map(|o| Self::start_output_group(o, group_queue_capacity, output_queue_capacity))
            .collect()
    }

    fn start_output_group(outputs: Vec<Box<Output>>, group_queue_capacity: usize,
        output_queue_capacity: usize) -> mpsc::Sender<Event>
    {
        let mut txs = Vec::new();

        for output in outputs {
            let (tx, rx) = mpsc::channel::<Event>(output_queue_capacity);
            executor::spawn(future::lazy(move || {
                info!("starting output");
                output.start()
                    // TODO handle output start failures.
                    .inspect_err(|e| error!("output start error: {:?}", e))
                    .map(|o| o.sink)
                    .and_then(move |output_sink| rx
                        .infallible()
                        // TODO don't fail the root future when output sink fails
                        .forward(output_sink)
                        .map(|_| {}))
                    .map_err(|e| error!("output send error: {:?}", e))
            }));
            txs.push(tx);
        }

        // Make broadcasting channel.
        let txs = Arc::new(txs);
        let (bcast_tx, bcast_rx) = mpsc::channel::<Event>(group_queue_capacity);
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
}