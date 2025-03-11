use crate::server::{
    itsi_server::Server, lifecycle_event::LifecycleEvent, listener::Listener,
    process_worker::ProcessWorker,
};
use itsi_error::{ItsiError, Result};
use itsi_rb_helpers::{call_without_gvl, create_ruby_thread};
use itsi_tracing::{error, info, warn};
use nix::libc::{self, exit};

use std::{
    sync::{atomic::AtomicUsize, Arc},
    time::{Duration, Instant},
};
use tokio::{
    runtime::{Builder as RuntimeBuilder, Runtime},
    sync::{broadcast, watch, Mutex},
    time::sleep,
};
use tracing::instrument;
pub(crate) struct ClusterMode {
    pub listeners: Arc<Vec<Arc<Listener>>>,
    pub server: Arc<Server>,
    pub process_workers: parking_lot::Mutex<Vec<ProcessWorker>>,
    pub lifecycle_channel: broadcast::Sender<LifecycleEvent>,
}

static WORKER_ID: AtomicUsize = AtomicUsize::new(0);
static CHILD_SIGNAL_SENDER: parking_lot::Mutex<Option<watch::Sender<()>>> =
    parking_lot::Mutex::new(None);

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
            .map(|_| ProcessWorker {
                worker_id: WORKER_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
                ..Default::default()
            })
            .collect();

        Self {
            listeners,
            server,
            process_workers: parking_lot::Mutex::new(process_workers),
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

    pub async fn handle_lifecycle_event(
        self: Arc<Self>,
        lifecycle_event: LifecycleEvent,
    ) -> Result<()> {
        match lifecycle_event {
            LifecycleEvent::Shutdown => {
                self.shutdown().await?;
                Ok(())
            }
            LifecycleEvent::Restart => {
                for worker in self.process_workers.lock().iter() {
                    worker.reboot(self.clone()).await
                }
                Ok(())
            }
            LifecycleEvent::IncreaseWorkers => {
                let mut workers = self.process_workers.lock();
                let worker = ProcessWorker {
                    worker_id: WORKER_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
                    ..Default::default()
                };
                let worker_clone = worker.clone();
                let self_clone = self.clone();
                create_ruby_thread(move || {
                    call_without_gvl(move || {
                        worker_clone.boot(self_clone);
                    })
                });
                workers.push(worker);
                Ok(())
            }
            LifecycleEvent::DecreaseWorkers => {
                let worker = {
                    let mut workers = self.process_workers.lock();
                    workers.pop()
                };
                if let Some(dropped_worker) = worker {
                    dropped_worker.request_shutdown();
                    let force_kill_time =
                        Instant::now() + Duration::from_secs_f64(self.server.shutdown_timeout);
                    while dropped_worker.is_alive() && force_kill_time > Instant::now() {
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                    if dropped_worker.is_alive() {
                        dropped_worker.force_kill();
                    }
                };
                Ok(())
            }
            LifecycleEvent::ForceShutdown => {
                for worker in self.process_workers.lock().iter() {
                    worker.force_kill();
                }
                unsafe { exit(0) };
                Ok(())
            }
        }
    }

    pub async fn shutdown(&self) -> Result<()> {
        let shutdown_timeout = self.server.shutdown_timeout;
        let workers = self.process_workers.lock().clone();

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

    pub fn receive_signal(signal: i32) {
        match signal {
            libc::SIGCHLD => {
                CHILD_SIGNAL_SENDER.lock().as_ref().inspect(|i| {
                    i.send(()).ok();
                });
            }
            _ => {
                // Handle other signals
            }
        }
    }

    #[instrument(skip(self), fields(mode = "cluster"))]
    pub fn run(self: Arc<Self>) -> Result<()> {
        self.process_workers
            .lock()
            .iter()
            .for_each(|worker| worker.boot(Arc::clone(&self)));

        let (sender, mut receiver) = watch::channel(());
        *CHILD_SIGNAL_SENDER.lock() = Some(sender);

        unsafe { libc::signal(libc::SIGCHLD, Self::receive_signal as usize) };

        let mut lifecycle_rx = self.lifecycle_channel.subscribe();
        let self_ref = self.clone();
        self.build_runtime().block_on(async {
            loop {
                tokio::select! {
                  _ = receiver.changed() => {
                    self_ref.process_workers.lock().iter().for_each(|worker| worker.boot_if_dead(Arc::clone(&self_ref)));
                  }
                  lifecycle_event = lifecycle_rx.recv() => match lifecycle_event{
                    Ok(lifecycle_event) => {
                      if let Err(e) = self_ref.clone().handle_lifecycle_event(lifecycle_event).await{
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
