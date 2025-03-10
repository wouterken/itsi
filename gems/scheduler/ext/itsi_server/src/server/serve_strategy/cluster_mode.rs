use crate::server::{
    lifecycle_event::LifecycleEvent, listener::Listener, process_worker::ProcessWorker,
    signal::handle_signals,
};
use itsi_error::{ItsiError, Result};
use itsi_tracing::{error, info, warn};
use magnus::{value::Opaque, Value};
use std::{num::NonZeroU8, sync::Arc, time::Duration};
use tokio::{
    runtime::{Builder as RuntimeBuilder, Runtime},
    sync::Mutex,
    time::sleep,
};
use tracing::instrument;
pub(crate) struct ClusterMode {
    pub app: Opaque<Value>,
    pub listeners: Arc<Vec<Arc<Listener>>>,
    pub script_name: String,
    pub thread_count: NonZeroU8,
    pub process_workers: Vec<ProcessWorker>,
    pub scheduler_class: Option<String>,
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
        script_name: String,
        thread_count: NonZeroU8,
        worker_count: NonZeroU8,
        scheduler_class: Option<String>,
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
            listeners,
            script_name,
            thread_count,
            process_workers,
            lifecycle,
            scheduler_class,
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
        match lifecycle_event {
            LifecycleEvent::Shutdown => self.shutdown().await,
            LifecycleEvent::Start => todo!(),
            LifecycleEvent::Restart => todo!(),
        }
    }

    pub async fn shutdown(&self) -> Result<()> {
        let shutdown_timeout = self.lifecycle.shutdown_timeout;
        let workers = self.process_workers.clone();

        workers.iter().for_each(|worker| worker.request_shutdown());

        let remaining_children = Arc::new(Mutex::new(workers.len()));
        let monitor_handle = {
            let remaining_children: Arc<Mutex<usize>> = Arc::clone(&remaining_children);
            let mut workers = workers.clone();
            tokio::spawn(async move {
                loop {
                    // Check if all workers have exited
                    let mut remaining = remaining_children.lock().await;
                    workers.retain(|worker| worker.is_alive());
                    *remaining = workers.len();
                    if *remaining == 0 {
                        break;
                    }
                    sleep(Duration::from_millis(100)).await;
                }
            })
        };

        tokio::select! {
            _ = monitor_handle => {
              info!("All children exited early, exit normally")
            }
            _ = sleep(Duration::from_secs_f64(shutdown_timeout)) => {
                warn!("Graceful shutdown timeout reached, force killing remaining children");
                workers.iter().for_each(|worker| worker.force_kill());
            }
        }

        Err(ItsiError::Break())
    }

    #[instrument(skip(self), fields(mode = "cluster"))]
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
                        Err(e) => error!("Error receiving lifecycle_event: {:?}", e),
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
