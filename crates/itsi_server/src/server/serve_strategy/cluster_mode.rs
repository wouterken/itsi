use crate::ruby_types::itsi_server::itsi_server_config::ItsiServerConfig;
use crate::server::signal::{subscribe_runtime_to_signals, unsubscribe_runtime};
use crate::server::{lifecycle_event::LifecycleEvent, process_worker::ProcessWorker};
use itsi_error::{ItsiError, Result};
use itsi_rb_helpers::{call_with_gvl, call_without_gvl, create_ruby_thread};
use itsi_tracing::{error, info, warn};
use magnus::Value;
use nix::{libc::exit, unistd::Pid};

use std::sync::atomic::{AtomicBool, Ordering};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    runtime::{Builder as RuntimeBuilder, Runtime},
    sync::{watch, Mutex},
    time::{self, sleep},
};
use tracing::{debug, instrument};
pub(crate) struct ClusterMode {
    pub server_config: Arc<ItsiServerConfig>,
    pub process_workers: parking_lot::Mutex<Vec<ProcessWorker>>,
}

static CHILD_SIGNAL_SENDER: parking_lot::Mutex<Option<watch::Sender<()>>> =
    parking_lot::Mutex::new(None);

static RELOAD_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

impl ClusterMode {
    pub fn new(server_config: Arc<ItsiServerConfig>) -> Self {
        let process_workers = (0..server_config.server_params.read().workers)
            .map(|id| ProcessWorker {
                worker_id: id as usize,
                ..Default::default()
            })
            .collect();

        Self {
            server_config,
            process_workers: parking_lot::Mutex::new(process_workers),
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

    pub fn invoke_hook(&self, hook_name: &str) {
        if let Some(hook) = self.server_config.server_params.read().hooks.get(hook_name) {
            call_with_gvl(|_| hook.call::<_, Value>(()).ok());
        }
    }

    fn next_worker_id(&self) -> usize {
        let mut ids: Vec<usize> = self
            .process_workers
            .lock()
            .iter()
            .map(|w| w.worker_id)
            .collect();
        self.next_available_id_in(&mut ids)
    }

    fn next_available_id_in(&self, list: &mut [usize]) -> usize {
        list.sort_unstable();
        for (expected, &id) in list.iter().enumerate() {
            if id != expected {
                return expected;
            }
        }
        list.len()
    }

    #[allow(clippy::await_holding_lock)]
    pub async fn handle_lifecycle_event(
        self: Arc<Self>,
        lifecycle_event: LifecycleEvent,
    ) -> Result<()> {
        match lifecycle_event {
            LifecycleEvent::Start => Ok(()),
            LifecycleEvent::PrintInfo => {
                self.print_info().await?;
                Ok(())
            }
            LifecycleEvent::Shutdown => {
                self.server_config.stop_watcher()?;
                self.shutdown().await?;
                self.invoke_hook("before_shutdown");
                Ok(())
            }
            LifecycleEvent::Restart => {
                if self.server_config.check_config().await {
                    self.invoke_hook("before_restart");
                    self.server_config.dup_fds()?;
                    self.shutdown().await.ok();
                    info!("Shutdown complete. Calling reload exec");
                    self.server_config.reload_exec()?;
                }
                Ok(())
            }
            LifecycleEvent::Reload => {
                if !self.server_config.check_config().await {
                    return Ok(());
                }
                let should_reexec = self.server_config.clone().reload(true)?;
                if should_reexec {
                    self.server_config.dup_fds()?;
                    self.shutdown().await.ok();
                    self.server_config.reload_exec()?;
                }

                if RELOAD_IN_PROGRESS
                    .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                    .is_err()
                {
                    warn!("Reload already in progress, ignoring request");
                    return Ok(());
                }
                let workers_to_load = self.server_config.server_params.read().workers;
                let mut next_workers = Vec::new();
                let mut old_workers = self.process_workers.lock().drain(..).collect::<Vec<_>>();

                // Spawn new workers
                for i in 0..workers_to_load {
                    let worker = ProcessWorker {
                        worker_id: i as usize,
                        ..Default::default()
                    };
                    let worker_clone = worker.clone();
                    let self_clone = self.clone();

                    call_with_gvl(|_| {
                        create_ruby_thread(move || {
                            call_without_gvl(move || match worker_clone.boot(self_clone) {
                                Err(err) => error!("Worker boot failed {:?}", err),
                                _ => {}
                            })
                        });
                    });

                    next_workers.push(worker);

                    if let Some(old) = old_workers.pop() {
                        old.graceful_shutdown(self.clone()).await;
                    }
                }

                for worker in old_workers {
                    worker.graceful_shutdown(self.clone()).await;
                }

                self.process_workers.lock().extend(next_workers);
                RELOAD_IN_PROGRESS.store(false, Ordering::SeqCst);

                Ok(())
            }
            LifecycleEvent::IncreaseWorkers => {
                let mut workers = self.process_workers.lock();
                let worker = ProcessWorker {
                    worker_id: self.next_worker_id(),
                    ..Default::default()
                };
                let worker_clone = worker.clone();
                let self_clone = self.clone();
                create_ruby_thread(move || {
                    call_without_gvl(move || {
                        worker_clone.boot(self_clone).ok();
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
                    let force_kill_time = Instant::now()
                        + Duration::from_secs_f64(
                            self.server_config.server_params.read().shutdown_timeout,
                        );
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
                error!("Force shutdown!");
                unsafe { exit(0) };
            }
            LifecycleEvent::ChildTerminated => {
                if RELOAD_IN_PROGRESS.load(Ordering::SeqCst) {
                    warn!("Reload already in progress, ignoring child signal");
                    return Ok(());
                }
                CHILD_SIGNAL_SENDER.lock().as_ref().inspect(|i| {
                    i.send(()).ok();
                });
                Ok(())
            }
        }
    }

    pub async fn shutdown(&self) -> Result<()> {
        let shutdown_timeout = self.server_config.server_params.read().shutdown_timeout;
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
              debug!("All children exited early, exit normally")
            }
            _ = sleep(Duration::from_secs_f64(shutdown_timeout)) => {
                warn!("Graceful shutdown timeout reached, force killing remaining children");
                workers.iter().for_each(|worker| worker.force_kill());
            }
        }

        Err(ItsiError::Break)
    }

    pub async fn print_info(self: Arc<Self>) -> Result<()> {
        println!("Itsi Cluster Info:");
        println!("Master PID: {:?}", Pid::this());
        if let Some(memory_limit) = self.server_config.server_params.read().worker_memory_limit {
            println!("Worker Memory Limit: {}", memory_limit);
        }

        if self.server_config.watcher_fd.is_some() {
            println!("File Watcher Enabled: true",);
            if let Some(watchers) = self
                .server_config
                .server_params
                .read()
                .notify_watchers
                .as_ref()
            {
                for watcher in watchers {
                    println!(
                        "Watching path: {} => {}",
                        watcher.0,
                        watcher
                            .1
                            .iter()
                            .map(|path| path.join(","))
                            .collect::<Vec<String>>()
                            .join(" ")
                    );
                }
            }
        }
        println!(
            "Silent Mode: {}",
            self.server_config.server_params.read().silence
        );
        println!(
            "Preload: {}",
            self.server_config.server_params.read().preload
        );
        let workers = self.process_workers.lock().clone();
        for worker in workers {
            worker.print_info()?;
            sleep(Duration::from_millis(50)).await;
        }
        Ok(())
    }

    pub fn stop(&self) -> Result<()> {
        for worker in self.process_workers.lock().iter() {
            if worker.is_alive() {
                worker.force_kill();
            }
        }
        Ok(())
    }

    #[instrument(skip(self), fields(mode = "cluster", pid=format!("{:?}", Pid::this())))]
    pub fn run(self: Arc<Self>) -> Result<()> {
        info!("Starting in Cluster mode");
        self.invoke_hook("before_fork");

        self.process_workers
            .lock()
            .iter()
            .try_for_each(|worker| worker.boot(Arc::clone(&self)))?;

        if cfg!(target_os = "linux") {
            self.server_config
                .server_params
                .write()
                .listeners
                .lock()
                .drain(..);
        };

        let (sender, mut receiver) = watch::channel(());
        *CHILD_SIGNAL_SENDER.lock() = Some(sender);

        let self_ref = self.clone();

        self.build_runtime().block_on(async {
          let mut lifecycle_rx = subscribe_runtime_to_signals();

          let self_ref = self_ref.clone();
          let memory_check_duration = if self_ref.server_config.server_params.read().worker_memory_limit.is_some(){
            time::Duration::from_secs(15)
          } else {
            time::Duration::from_secs(60 * 60 * 24 * 365 * 100)
          };

          let mut memory_check_interval = time::interval(memory_check_duration);

          self.invoke_hook("after_start");

          loop {
            tokio::select! {
              _ = receiver.changed() => {
                let mut workers = self_ref.process_workers.lock();
                workers.retain(|worker| {
                  worker.boot_if_dead(self_ref.clone())
                });
                if workers.is_empty() {
                    warn!("No workers running. Send SIGTTIN to increase worker count");
                }
              }
              _ = memory_check_interval.tick() => {
                let worker_memory_limit = self_ref.server_config.server_params.read().worker_memory_limit;
                if let Some(memory_limit) = worker_memory_limit {
                  let largest_worker = {
                    let workers = self_ref.process_workers.lock();
                    workers.iter().max_by(|wa, wb| wa.memory_usage().cmp(&wb.memory_usage())).cloned()
                  };
                  if let Some(largest_worker) = largest_worker {
                    if let Some(current_mem_usage) = largest_worker.memory_usage(){
                      if current_mem_usage > memory_limit {
                        largest_worker.reboot(self_ref.clone()).await.ok();
                        if let Some(hook) = self_ref.server_config.server_params.read().hooks.get("after_memory_limit_reached") {
                          call_with_gvl(|_|  hook.call::<_, Value>((largest_worker.pid(),)).ok() );
                        }
                      }
                    }
                  }
                }
              }
              lifecycle_event = lifecycle_rx.recv() => match lifecycle_event{
                Ok(lifecycle_event) => {
                  if let Err(e) = self_ref.clone().handle_lifecycle_event(lifecycle_event).await{
                    match e {
                      ItsiError::Break => break,
                      _ => error!("Error in handle_lifecycle_event {:?}", e)
                    }
                  }

                },
                Err(e) => {
                  error!("Error receiving lifecycle_event: {:?}", e);
                  break
                },
              }
            }
          }
        });

        unsubscribe_runtime();
        self.server_config
            .server_params
            .write()
            .listeners
            .lock()
            .drain(..);
        Ok(())
    }
}
