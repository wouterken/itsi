use super::{bind::Bind, listener::Listener};
use crate::{
    request::itsi_request::ItsiRequest,
    server::{
        lifecycle_event::LifecycleEvent,
        serve_strategy::{ServeStrategy, SingleMode},
        signal::handle_signals,
        thread_worker::ThreadWorker,
    },
};
use derive_more::Debug;
use hyper_util::{rt::TokioExecutor, server::conn::auto::Builder};
use itsi_error::ItsiError;
use itsi_rb_helpers::call_without_gvl;
use itsi_tracing::{error, info};
use magnus::{
    error::Result,
    scan_args::{get_kwargs, scan_args, Args, KwArgs},
    value::Opaque,
    RHash, Value,
};
use parking_lot::Mutex;
use std::{cmp::max, sync::Arc};
use tokio::runtime::{Builder as RuntimeBuilder, Runtime};
use tokio::task::JoinSet;

static DEFAULT_BIND: &str = "localhost:3000";

#[magnus::wrap(class = "Itsi::Server", free_immediately, size)]
#[derive(Debug)]
pub struct Server {
    #[debug(skip)]
    app: Opaque<Value>,
    #[allow(unused)]
    workers: u16,
    #[allow(unused)]
    threads: u16,
    #[allow(unused)]
    shutdown_timeout: f64,
    script_name: String,
    pub(crate) binds: Mutex<Vec<Bind>>,
}

pub enum RequestJob {
    ProcessRequest(ItsiRequest),
    Shutdown,
}

impl Server {
    pub fn new(args: &[Value]) -> Result<Self> {
        type OptionalArgs = (
            Option<u16>,
            Option<u16>,
            Option<f64>,
            Option<String>,
            Option<Vec<String>>,
        );

        let scan_args: Args<(), (), (), (), RHash, ()> = scan_args(args)?;
        let args: KwArgs<(Value,), OptionalArgs, ()> = get_kwargs(
            scan_args.keywords,
            &["app"],
            &[
                "workers",
                "threads",
                "shutdown_timeout",
                "script_name",
                "binds",
            ],
        )?;
        let server = Server {
            app: Opaque::from(args.required.0),
            workers: max(args.optional.0.unwrap_or(1), 1),
            threads: max(args.optional.1.unwrap_or(1), 1),
            shutdown_timeout: args.optional.2.unwrap_or(5.0),
            script_name: args.optional.3.unwrap_or("".to_string()),
            binds: Mutex::new(
                args.optional
                    .4
                    .unwrap_or_else(|| vec![DEFAULT_BIND.to_string()])
                    .into_iter()
                    .map(|s| s.parse().unwrap_or_else(|_| Bind::default()))
                    .collect(),
            ),
        };
        Ok(server)
    }

    pub fn build_runtime(&self) -> Runtime {
        let mut builder: RuntimeBuilder = RuntimeBuilder::new_current_thread();
        builder
            .thread_name("itsi-server-accept-loop")
            .thread_stack_size(3 * 1024 * 1024)
            .enable_io()
            .enable_time()
            .build()
            .expect("Failed to build Tokio runtime")
    }

    pub fn build_thread_workers(
        &self,
    ) -> (
        Arc<Vec<ThreadWorker>>,
        Arc<crossbeam::channel::Sender<RequestJob>>,
    ) {
        let (sender, receiver) = crossbeam::channel::bounded(1000);
        let receiver_ref = Arc::new(receiver);
        let sender_ref = Arc::new(sender);
        (
            Arc::new(
                (1..=self.threads)
                    .map(|id| {
                        info!("Creating worker thread {}", id);
                        ThreadWorker::new(id, self.app, receiver_ref.clone(), sender_ref.clone())
                    })
                    .collect::<Vec<_>>(),
            ),
            sender_ref,
        )
    }

    pub(crate) fn build_listeners(&self) -> Vec<Listener> {
        self.binds
            .lock()
            .iter()
            .cloned()
            .map(Listener::from)
            .collect::<Vec<_>>()
    }

    pub(crate) fn build_strategy(&self) -> ServeStrategy {
        let server = Builder::new(TokioExecutor::new());
        let (workers, sender) = self.build_thread_workers();
        ServeStrategy::Single(Arc::new(SingleMode {
            server,
            workers,
            sender,
            script_name: self.script_name.clone(),
            shutdown_timeout: self.shutdown_timeout,
        }))
    }

    pub fn start(&self) {
        info!(
            "Starting Itsi Server on {:?}. Threads: {}",
            self.binds.lock(),
            self.threads
        );

        call_without_gvl(|| {
            let (lifecycle_tx, _) = tokio::sync::broadcast::channel::<LifecycleEvent>(100);
            let lifecycle_tx = Arc::new(lifecycle_tx);
            let strategy = Arc::new(self.build_strategy());
            info!("Initialized strategy");
            let mut listener_task_set = JoinSet::new();

            self.build_runtime().block_on(async {
                let signals_task = tokio::spawn(handle_signals(lifecycle_tx.clone()));
                info!("Initialized signals task");
                for listener in self.build_listeners() {
                    info!("Initialized listener");
                    let listener = Arc::new(listener);
                    let strategy = strategy.clone();
                    let mut lifecycle_rx = lifecycle_tx.subscribe();
                    info!("Initialized listener");
                    listener_task_set.spawn(async move {
                        let listener = listener.clone();
                        let strategy = strategy.clone();
                        loop {
                          info!("In select loop");
                            tokio::select! {
                                accept_result = listener.accept() => match accept_result {
                                  Ok(accept_result) => {
                                    if let Err(e) = strategy.serve_connection(accept_result, listener.clone()){
                                      error!("Error in serve_connection {:?}", e)
                                    }
                                  },
                                  Err(e) => error!("Error in listener.accept {:?}", e),
                              },
                                lifecycle_event = lifecycle_rx.recv() => match lifecycle_event{
                                  Ok(lifecycle_event) => {
                                    if let Err(e) = strategy.handle_lifecycle_event(lifecycle_event).await{
                                      match e {
                                        ItsiError::Break() => break,
                                        _ => error!("Error in handle_lifecycle_event {:?}", e)
                                      }
                                    }

                                  },
                                  Err(e) => error!("Error receiving lifecycle_event: {:?}", e),
                              }
                            }
                        }
                    });
                }

                while let Some(_res) = listener_task_set.join_next().await {}
                if let Err(e) =  signals_task.await {
                    error!("Error closing server: {:?}", e);
                }
            })
        });
    }
}
