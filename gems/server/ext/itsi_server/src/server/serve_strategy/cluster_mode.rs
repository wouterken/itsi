use crate::server::{
    lifecycle_event::{self, LifecycleEvent},
    listener::Listener,
    process_worker::ProcessWorker,
    signal::handle_signals,
};
use hyper_util::{rt::TokioExecutor, server::conn::auto::Builder};
use itsi_error::{ItsiError, Result};
use itsi_tracing::{error, info};
use magnus::{value::Opaque, Value};
use std::{num::NonZeroU8, sync::Arc};
use tokio::runtime::{Builder as RuntimeBuilder, Runtime};

pub(crate) struct ClusterMode {
    pub app: Opaque<Value>,
    pub server: Builder<TokioExecutor>,
    pub listeners: Arc<Vec<Arc<Listener>>>,
    pub script_name: String,
    pub thread_count: NonZeroU8,
    pub process_workers: Vec<ProcessWorker>,
    pub lifecycle: ClusterLifecycle,
}

pub(crate) struct ClusterLifecycle {
    pub before_fork: Option<Box<dyn FnOnce() + Send + Sync>>,
    pub after_fork: Arc<Option<Box<dyn Fn() + Send + Sync>>>,
    pub shutdown_timeout: f64,
}

impl ClusterMode {
    pub fn new(
        app: Opaque<Value>,
        listeners: Arc<Vec<Arc<Listener>>>,
        server: Builder<TokioExecutor>,
        script_name: String,
        thread_count: NonZeroU8,
        worker_count: NonZeroU8,
        mut lifecycle: ClusterLifecycle,
    ) -> Self {
        if let Some(f) = lifecycle.before_fork.take() {
            f();
        }
        let process_workers = (0..worker_count.get())
            .map(|worker_id| ProcessWorker {
                worker_id,
                ..Default::default()
            })
            .collect();

        Self {
            app,
            server,
            listeners,
            script_name,
            thread_count,
            process_workers,
            lifecycle,
        }
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

    pub async fn handle_lifecycle_event(&self, lifecycle_event: LifecycleEvent) -> Result<()> {
        self.process_workers
            .iter()
            .for_each(|worker| worker.shutdown());
        Err(ItsiError::Break())
    }

    pub fn run(self: Arc<Self>) -> Result<()> {
        self.process_workers
            .iter()
            .for_each(|worker| worker.boot(Arc::clone(&self)));

        let (lifecycle_tx, mut lifecycle_rx) =
            tokio::sync::broadcast::channel::<LifecycleEvent>(100);
        let lifecycle_tx = Arc::new(lifecycle_tx);

        self.build_runtime().block_on(async {
            let signals_task = tokio::spawn(handle_signals(lifecycle_tx));
            loop {
                tokio::select! {
                      lifecycle_event = lifecycle_rx.recv() => match lifecycle_event{
                        Ok(lifecycle_event) => {
                          if let Err(e) = self.handle_lifecycle_event(lifecycle_event).await{
                            match e {
                              ItsiError::Break() => break,
                              _ => error!("Error in handle_lifecycle_event {:?}", e)
                            }
                          }

                        },
                        Err(e) => (),//error!("Error receiving lifecycle_event: {:?}", e),
                    }
                }
            }

            if let Err(e) = signals_task.await {
                error!("Error closing server: {:?}", e);
            }
        });

        Ok(())
    }
}
