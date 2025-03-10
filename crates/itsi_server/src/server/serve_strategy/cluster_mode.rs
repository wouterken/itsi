use crate::server::{
    itsi_server::Server, lifecycle_event::LifecycleEvent, listener::Listener,
    process_worker::ProcessWorker,
};
use itsi_error::{ItsiError, Result};
use itsi_tracing::{error, info, warn};
use std::{sync::Arc, time::Duration};
use tokio::{
    runtime::{Builder as RuntimeBuilder, Runtime},
    sync::{broadcast, Mutex},
    time::sleep,
};
use tracing::instrument;
pub(crate) struct ClusterMode {
    pub listeners: Arc<Vec<Arc<Listener>>>,
    pub server: Arc<Server>,
    pub process_workers: Vec<ProcessWorker>,
    pub lifecycle_channel: broadcast::Sender<LifecycleEvent>,
}

impl ClusterMode {
    pub fn new(
        server: Arc<Server>,
        listeners: Arc<Vec<Arc<Listener>>>,
        lifecycle_channel: broadcast::Sender<LifecycleEvent>,
    ) -> Self {
        if let Some(f) = server.before_fork.lock().take() {
            f();
        }
        let process_workers = (0..server.workers)
            .map(|worker_id| ProcessWorker {
                worker_id,
                ..Default::default()
            })
            .collect();

        Self {
            listeners,
            server,
            process_workers,
            lifecycle_channel,
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
            LifecycleEvent::IncreaseWorkers => todo!(),
            LifecycleEvent::DecreaseWorkers => todo!(),
            LifecycleEvent::RestartWorkers => todo!(),
            LifecycleEvent::RestartWorkersFreshConfig => todo!(),
        }
    }

    pub async fn shutdown(&self) -> Result<()> {
        let shutdown_timeout = self.server.shutdown_timeout;
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

        let mut lifecycle_rx = self.lifecycle_channel.subscribe();

        self.build_runtime().block_on(async {
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
        });

        Ok(())
    }
}
