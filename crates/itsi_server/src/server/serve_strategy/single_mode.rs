use crate::{
    ruby_types::itsi_server::itsi_server_config::ItsiServerConfig,
    server::{
        lifecycle_event::LifecycleEvent,
        request_job::RequestJob,
        serve_strategy::acceptor::{Acceptor, AcceptorArgs},
        signal::{
            send_lifecycle_event, subscribe_runtime_to_signals, unsubscribe_runtime,
            SHUTDOWN_REQUESTED,
        },
        thread_worker::{build_thread_workers, ThreadWorker},
    },
};
use hyper_util::{
    rt::{TokioExecutor, TokioTimer},
    server::conn::auto::Builder,
};
use itsi_error::{ItsiError, Result};
use itsi_rb_helpers::{
    call_with_gvl, call_without_gvl, create_ruby_thread, funcall_no_ret, print_rb_backtrace,
};
use itsi_tracing::{debug, error, info};
use magnus::{value::ReprValue, Value};
use nix::unistd::Pid;
use parking_lot::RwLock;
use std::sync::Arc;
use std::{
    collections::HashMap,
    sync::atomic::{AtomicBool, Ordering},
    thread::sleep,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::{
    runtime::{Builder as RuntimeBuilder, Runtime},
    sync::watch::{self},
    task::JoinSet,
};
use tracing::instrument;

pub struct SingleMode {
    pub worker_id: usize,
    pub executor: Builder<TokioExecutor>,
    pub server_config: Arc<ItsiServerConfig>,
    pub restart_requested: AtomicBool,
    pub status: RwLock<HashMap<u8, (u64, u64)>>,
}

#[derive(PartialEq, Debug)]
pub enum RunningPhase {
    Running,
    ShutdownPending,
    Shutdown,
}

impl SingleMode {
    #[instrument(parent=None, skip_all)]
    pub fn new(server_config: Arc<ItsiServerConfig>, worker_id: usize) -> Result<Self> {
        server_config.server_params.read().preload_ruby()?;
        let executor = {
            let mut executor = Builder::new(TokioExecutor::new());
            let server_params = server_config.server_params.read();
            let mut http1_executor = executor.http1();

            http1_executor
                .header_read_timeout(server_params.header_read_timeout)
                .pipeline_flush(server_params.pipeline_flush)
                .timer(TokioTimer::new());

            if let Some(writev) = server_params.writev {
                http1_executor.writev(writev);
            }

            executor
                .http2()
                .max_concurrent_streams(server_params.max_concurrent_streams)
                .max_local_error_reset_streams(server_params.max_local_error_reset_streams)
                .max_header_list_size(server_params.max_header_list_size)
                .max_send_buf_size(server_params.max_send_buf_size)
                .enable_connect_protocol();
            executor
        };

        Ok(Self {
            worker_id,
            executor,
            server_config,
            restart_requested: AtomicBool::new(false),
            status: RwLock::new(HashMap::new()),
        })
    }

    pub fn is_zero_worker(&self) -> bool {
        self.worker_id == 0
    }

    pub fn build_runtime(&self) -> Runtime {
        let mut builder: RuntimeBuilder = if self
            .server_config
            .server_params
            .read()
            .multithreaded_reactor
        {
            RuntimeBuilder::new_multi_thread()
        } else {
            RuntimeBuilder::new_current_thread()
        };
        builder
            .thread_name("itsi-server-accept-loop")
            .thread_stack_size(512 * 1024)
            .max_blocking_threads(4)
            .event_interval(16)
            .global_queue_interval(64)
            .max_io_events_per_tick(256)
            .enable_all()
            .build()
            .expect("Failed to build Tokio runtime")
    }

    pub fn stop(&self) -> Result<()> {
        SHUTDOWN_REQUESTED.store(true, std::sync::atomic::Ordering::SeqCst);
        send_lifecycle_event(LifecycleEvent::Shutdown);
        Ok(())
    }

    pub async fn print_info(&self, thread_workers: Arc<Vec<Arc<ThreadWorker>>>) -> Result<()> {
        println!(" └─ Worker");
        println!(
            "    - binds: {:?}",
            self.server_config.server_params.read().binds
        );

        println!(
            "    ─ streaming body: {:?}",
            self.server_config.server_params.read().streamable_body
        );
        println!(
            "    ─ multithreaded runtime: {:?}",
            self.server_config
                .server_params
                .read()
                .multithreaded_reactor
        );
        println!(
            "    ─ scheduler: {:?}",
            self.server_config.server_params.read().scheduler_class
        );
        println!(
            "    ─ OOB GC Response threadhold: {:?}",
            self.server_config
                .server_params
                .read()
                .oob_gc_responses_threshold
        );
        for worker in thread_workers.iter() {
            println!("   └─ - Thread : {:?}", worker.id);
            println!("       - # Requests Processed: {:?}", worker.request_id);
            println!(
                "       - Last Request Started: {:?} ago",
                if worker.current_request_start.load(Ordering::Relaxed) == 0 {
                    Duration::from_secs(0)
                } else {
                    SystemTime::now()
                        .duration_since(
                            UNIX_EPOCH
                                + Duration::from_secs(
                                    worker.current_request_start.load(Ordering::Relaxed),
                                ),
                        )
                        .unwrap_or(Duration::from_secs(0))
                }
            );
            call_with_gvl(|_| {
                if let Some(thread) = worker.thread.read().as_ref() {
                    if let Ok(backtrace) = thread.funcall::<_, _, Vec<String>>("backtrace", ()) {
                        println!("       - Backtrace:");
                        for line in backtrace {
                            println!("       -   {}", line);
                        }
                    }
                }
            })
        }

        Ok(())
    }

    pub fn start_monitors(
        self: Arc<Self>,
        thread_workers: Arc<Vec<Arc<ThreadWorker>>>,
    ) -> Option<magnus::Thread> {
        call_with_gvl(move |_| {
            create_ruby_thread(move || {
                call_without_gvl(move || {
                    let monitor_runtime = RuntimeBuilder::new_current_thread()
                        .enable_all()
                        .build()
                        .unwrap();
                    let receiver = self.clone();
                    monitor_runtime.block_on({
                        let mut lifecycle_rx = subscribe_runtime_to_signals();
                        let receiver = receiver.clone();
                        let thread_workers = thread_workers.clone();
                        async move {
                            loop {
                                tokio::select! {
                                  _ = tokio::time::sleep(Duration::from_secs(1)) => {
                                    let mut status_lock = receiver.status.write();
                                    thread_workers.iter().for_each(|worker| {
                                        let worker_entry = status_lock.entry(worker.id);
                                        let data = (
                                            worker.request_id.load(Ordering::Relaxed),
                                            worker.current_request_start.load(Ordering::Relaxed),
                                        );
                                        worker_entry.or_insert(data);
                                    });
                                  }
                                  lifecycle_event = lifecycle_rx.recv() => {
                                      match lifecycle_event {
                                          Ok(LifecycleEvent::Restart) | Ok(LifecycleEvent::Reload) => {
                                              receiver.restart().await.ok();
                                          }
                                          Ok(LifecycleEvent::Shutdown) => {
                                            break;
                                          }
                                          Ok(LifecycleEvent::PrintInfo) => {
                                            receiver.print_info(thread_workers.clone()).await.ok();
                                          }
                                          Err(e) => {
                                            debug!("Lifecycle channel closed: {:?}, exiting single mode monitor loop", e);
                                            break;
                                          }
                                          _ => {}
                                      }
                                  }
                                }
                            }
                        }
                    })
                })
            })
        })
    }

    #[instrument(name="worker", parent=None, skip(self), fields(pid=format!("{}", Pid::this())))]
    pub fn run(self: Arc<Self>) -> Result<()> {
        let (thread_workers, job_sender, nonblocking_sender) = build_thread_workers(
            self.server_config.server_params.read().clone(),
            self.worker_id,
        )
        .inspect_err(|e| {
            if let Some(err_val) = e.value() {
                print_rb_backtrace(err_val);
            }
        })?;

        let worker_count = thread_workers.len();
        info!(
            threads = worker_count,
            binds = format!("{:?}", self.server_config.server_params.read().binds)
        );

        let shutdown_timeout = self.server_config.server_params.read().shutdown_timeout;
        let (shutdown_sender, _) = watch::channel(RunningPhase::Running);
        let monitor_thread = self.clone().start_monitors(thread_workers.clone());

        // If we're on Linux with reuse_port enabled, we can use
        // kernel level load balancing across processes sharing a port.
        // To take advantage of this, these forks will rebind to the same port upon boot.
        // Worker 0 is special (this one just inherits the bind from the master process).
        let is_zero_worker = self.is_zero_worker();
        let should_rebind = !is_zero_worker || self.server_config.use_reuse_port_load_balancing();

        if monitor_thread.is_none() {
            error!("Failed to start monitor thread");
            return Err(ItsiError::new("Failed to start monitor thread"));
        }
        let monitor_thread = monitor_thread.unwrap();
        if SHUTDOWN_REQUESTED.load(Ordering::SeqCst) {
            return Ok(());
        }
        let runtime = self.build_runtime();
        let result = runtime.block_on(async {
            let mut listener_task_set = JoinSet::new();
            let server_params = self.server_config.server_params.read().clone();
            if let Err(err) = server_params.initialize_middleware().await {
                error!("Failed to initialize middleware: {}", err);
                return Err(ItsiError::new("Failed to initialize middleware"));
            }
            let tokio_listeners = server_params
                .listeners
                .lock()
                .drain(..)
                .map(|list| Arc::new(list.into_tokio_listener(should_rebind)))
                .collect::<Vec<_>>();

            tokio_listeners.iter().cloned().for_each(|listener| {
                let shutdown_sender = shutdown_sender.clone();
                let job_sender = job_sender.clone();
                let nonblocking_sender = nonblocking_sender.clone();

                let mut lifecycle_rx = subscribe_runtime_to_signals();
                let mut shutdown_receiver = shutdown_sender.subscribe();
                let mut acceptor = Acceptor {
                    acceptor_args: Arc::new(AcceptorArgs {
                        strategy: self.clone(),
                        listener_info: listener.listener_info(),
                        shutdown_receiver: shutdown_sender.subscribe(),
                        job_sender: job_sender.clone(),
                        nonblocking_sender: nonblocking_sender.clone(),
                        server_params: server_params.clone(),
                    }),
                    join_set: JoinSet::new(),
                };

                let shutdown_rx_for_acme_task = shutdown_receiver.clone();
                let acme_task_listener_clone = listener.clone();

                let mut after_accept_wait: Option<Duration> = None::<Duration>;

                if cfg!(target_os = "macos") {
                    after_accept_wait = if server_params.workers > 1 {
                        Some(Duration::from_nanos(10 * server_params.workers as u64))
                    } else {
                        None
                    };
                };

                listener_task_set.spawn(async move {
                    acme_task_listener_clone
                        .spawn_acme_event_task(shutdown_rx_for_acme_task)
                        .await;
                });

                listener_task_set.spawn(async move {
                    loop {
                        // Process any pending signals before select
                        tokio::select! {
                            accept_result = listener.accept() => {
                                match accept_result {
                                    Ok(accepted) => acceptor.serve_connection(accepted).await,
                                    Err(e) => debug!("Listener.accept failed: {:?}", e)
                                }
                                if cfg!(target_os = "macos") {
                                  if let Some(after_accept_wait) = after_accept_wait{
                                    tokio::time::sleep(after_accept_wait).await;
                                  }
                                }
                            },
                            _ = shutdown_receiver.changed() => {
                                debug!("Shutdown requested via receiver");
                                break;
                            },
                            lifecycle_event = lifecycle_rx.recv() => {
                                match lifecycle_event {
                                    Ok(LifecycleEvent::Shutdown) => {
                                        debug!("Received LifecycleEvent::Shutdown");
                                        let _ = shutdown_sender.send(RunningPhase::ShutdownPending);
                                        break;
                                    },
                                    Err(e) => {
                                      debug!("Lifecycle channel closed: {:?}, exiting accept loop", e);
                                      break
                                    },
                                    _ => ()
                                }
                            }
                        }
                    }
                    acceptor.join().await;
                });
            });

            if self.is_single_mode() {
                self.invoke_hook("after_start");
            }

            while let Some(_res) = listener_task_set.join_next().await {}
            drop(tokio_listeners);

            Ok::<(), ItsiError>(())
        });

        debug!("Single mode runtime exited.");

        for _i in 0..thread_workers.len() {
            job_sender.send_blocking(RequestJob::Shutdown).unwrap();
            nonblocking_sender
                .send_blocking(RequestJob::Shutdown)
                .unwrap();
        }
        if result.is_err() {
            send_lifecycle_event(LifecycleEvent::Shutdown);
        }

        shutdown_sender.send(RunningPhase::Shutdown).ok();
        runtime.shutdown_timeout(Duration::from_millis(100));
        unsubscribe_runtime();

        debug!("Shutdown timeout finished.");

        let deadline = Instant::now() + Duration::from_secs_f64(shutdown_timeout);
        loop {
            if thread_workers
                .iter()
                .all(|worker| call_with_gvl(move |_| !worker.poll_shutdown(deadline)))
            {
                funcall_no_ret(monitor_thread, "join", ()).ok();
                break;
            }
            sleep(Duration::from_millis(50));
        }

        if self.is_single_mode() {
            self.invoke_hook("before_shutdown");
        }

        if self.restart_requested.load(Ordering::SeqCst) {
            self.restart_requested.store(false, Ordering::SeqCst);
            info!("Worker restarting");
            self.run()?;
        }

        debug!("Runtime has shut down");
        result
    }

    pub fn is_single_mode(&self) -> bool {
        self.server_config.server_params.read().workers == 1
    }

    pub fn invoke_hook(&self, hook_name: &str) {
        if let Some(hook) = self.server_config.server_params.read().hooks.get(hook_name) {
            call_with_gvl(|_| hook.call::<_, Value>(()).ok());
        }
    }
    /// Restart the server while keeping connections open.
    pub async fn restart(&self) -> Result<()> {
        if !self.server_config.check_config().await {
            return Ok(());
        }
        if self.is_single_mode() {
            self.invoke_hook("before_restart");
        }
        self.server_config.dup_fds()?;
        self.server_config.reload_exec()?;
        Ok(())
    }
}
