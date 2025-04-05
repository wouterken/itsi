use crate::{
    ruby_types::itsi_server::itsi_server_config::ItsiServerConfig,
    server::{
        binds::listener::ListenerInfo,
        io_stream::IoStream,
        lifecycle_event::LifecycleEvent,
        request_job::RequestJob,
        signal::{SHUTDOWN_REQUESTED, SIGNAL_HANDLER_CHANNEL},
        thread_worker::{build_thread_workers, ThreadWorker},
    },
    services::itsi_http_service::{ItsiHttpService, ItsiHttpServiceInner},
};
use hyper_util::{
    rt::{TokioExecutor, TokioIo, TokioTimer},
    server::conn::auto::Builder,
};
use itsi_error::{ItsiError, Result};
use itsi_rb_helpers::{
    call_with_gvl, call_without_gvl, create_ruby_thread, funcall_no_ret, print_rb_backtrace,
};
use itsi_tracing::{debug, error, info};
use magnus::value::ReprValue;
use nix::unistd::Pid;
use parking_lot::RwLock;
use std::{
    collections::HashMap,
    num::NonZeroU8,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::sleep,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::{
    runtime::{Builder as RuntimeBuilder, Runtime},
    sync::{
        broadcast,
        watch::{self},
    },
    task::JoinSet,
};
use tracing::instrument;

pub struct SingleMode {
    pub executor: Builder<TokioExecutor>,
    pub server_config: Arc<ItsiServerConfig>,
    pub(crate) lifecycle_channel: broadcast::Sender<LifecycleEvent>,
    pub restart_requested: AtomicBool,
    pub status: RwLock<HashMap<u8, (u64, u64)>>,
}

pub enum RunningPhase {
    Running,
    ShutdownPending,
    Shutdown,
}

impl SingleMode {
    #[instrument(parent=None, skip_all)]
    pub fn new(server_config: Arc<ItsiServerConfig>) -> Result<Self> {
        server_config.server_params.read().preload_ruby()?;
        Ok(Self {
            executor: Builder::new(TokioExecutor::new()),
            server_config,
            lifecycle_channel: SIGNAL_HANDLER_CHANNEL.0.clone(),
            restart_requested: AtomicBool::new(false),
            status: RwLock::new(HashMap::new()),
        })
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
            .thread_stack_size(3 * 1024 * 1024)
            .enable_io()
            .enable_time()
            .build()
            .expect("Failed to build Tokio runtime")
    }

    pub fn stop(&self) -> Result<()> {
        self.lifecycle_channel.send(LifecycleEvent::Shutdown).ok();
        Ok(())
    }

    pub async fn print_info(&self, thread_workers: Arc<Vec<Arc<ThreadWorker>>>) -> Result<()> {
        println!(" └─ Worker");
        println!(
            "    - binds: {:?}",
            self.server_config.server_params.read().binds
        );

        println!(
            "    ─ script_name: {:?}",
            self.server_config.server_params.read().script_name
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
    ) -> magnus::Thread {
        call_with_gvl(move |_| {
            create_ruby_thread(move || {
                call_without_gvl(move || {
                    let monitor_runtime = RuntimeBuilder::new_current_thread()
                        .enable_time()
                        .build()
                        .unwrap();
                    let receiver = self.clone();
                    monitor_runtime.block_on({
                        let mut lifecycle_rx = receiver.lifecycle_channel.subscribe();
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
                                          Ok(LifecycleEvent::Restart) => {
                                              receiver.restart().ok();
                                          }
                                          Ok(LifecycleEvent::Reload) => {
                                              receiver.reload().ok();
                                          }
                                          Ok(LifecycleEvent::Shutdown) => {
                                            break;
                                          }
                                          Ok(LifecycleEvent::PrintInfo) => {
                                            receiver.print_info(thread_workers.clone()).await.ok();
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
        let mut listener_task_set = JoinSet::new();
        let runtime = self.build_runtime();

        let (thread_workers, job_sender) = build_thread_workers(
            self.server_config.server_params.read().clone(),
            Pid::this(),
            NonZeroU8::try_from(self.server_config.server_params.read().threads).unwrap(),
        )
        .inspect_err(|e| {
            if let Some(err_val) = e.value() {
                print_rb_backtrace(err_val);
            }
        })?;

        info!(
            threads = thread_workers.len(),
            binds = format!("{:?}", self.server_config.server_params.read().binds)
        );

        let (shutdown_sender, _) = watch::channel(RunningPhase::Running);
        let thread = self.clone().start_monitors(thread_workers.clone());
        if SHUTDOWN_REQUESTED.load(Ordering::SeqCst) {
            return Ok(());
        }
        runtime.block_on(
          async  {
              let server_params = self.server_config.server_params.read().clone();
              server_params.middleware.get().unwrap().initialize_layers().await?;
              let tokio_listeners = server_params.listeners.lock()
                  .drain(..)
                  .map(|list| {
                    Arc::new(list.into_tokio_listener())
                  })
                  .collect::<Vec<_>>();

              for listener in tokio_listeners.iter() {
                  let mut lifecycle_rx = self.lifecycle_channel.subscribe();

                  let listener_info = Arc::new(listener.listener_info());
                  let self_ref = self.clone();
                  let listener = listener.clone();
                  let shutdown_sender = shutdown_sender.clone();
                  let job_sender = job_sender.clone();
                  let workers_clone = thread_workers.clone();
                  let listener_clone = listener.clone();
                  let mut shutdown_receiver = shutdown_sender.subscribe();
                  let shutdown_receiver_clone = shutdown_receiver.clone();
                  listener_task_set.spawn(async move {
                    listener_clone.spawn_state_task(shutdown_receiver_clone).await;
                  });

                  listener_task_set.spawn(async move {
                    let strategy_clone = self_ref.clone();
                    let mut acceptor_task_set = JoinSet::new();
                    loop {
                        tokio::select! {
                            accept_result = listener.accept() => match accept_result {
                              Ok(accept_result) => {
                                let strategy = strategy_clone.clone();
                                let listener_info = listener_info.clone();
                                let shutdown_receiver = shutdown_receiver.clone();
                                let job_sender = job_sender.clone();
                                acceptor_task_set.spawn(async move {
                                  strategy.serve_connection(accept_result, job_sender, listener_info, shutdown_receiver).await;
                                });
                              },
                              Err(e) => debug!("Listener.accept failed {:?}", e),
                            },
                            _ = shutdown_receiver.changed() => {
                              break;
                            }
                            lifecycle_event = lifecycle_rx.recv() => match lifecycle_event{
                              Ok(LifecycleEvent::Shutdown) => {
                                shutdown_sender.send(RunningPhase::ShutdownPending).unwrap();
                                // Tell any in-progress connections to stop accepting new requests
                                tokio::time::sleep(Duration::from_millis(25)).await;
                                // Tell workers to stop processing requests once they've flushed their buffers.
                                for _i in 0..workers_clone.len() {
                                  job_sender.send(RequestJob::Shutdown).await.unwrap();
                                }
                                break;
                              },
                              Err(e) => error!("Error receiving lifecycle_event: {:?}", e),
                              _ => {}
                          }
                        }
                    }
                    while let Some(_res) = acceptor_task_set.join_next().await {}
                });

              }

              while let Some(_res) = listener_task_set.join_next().await {}

              // Explicitly drop all listeners to ensure file descriptors are released
              drop(tokio_listeners);

              Ok::<(), ItsiError>(())
          })?;

        shutdown_sender.send(RunningPhase::Shutdown).ok();
        let deadline = Instant::now()
            + Duration::from_secs_f64(self.server_config.server_params.read().shutdown_timeout);

        runtime.shutdown_timeout(Duration::from_millis(100));

        loop {
            if thread_workers
                .iter()
                .all(|worker| call_with_gvl(move |_| !worker.poll_shutdown(deadline)))
            {
                funcall_no_ret(thread, "join", ()).ok();
                break;
            }
            sleep(Duration::from_millis(50));
        }

        if self.restart_requested.load(Ordering::SeqCst) {
            self.restart_requested.store(false, Ordering::SeqCst);
            info!("Worker restarting");
            self.run()?;
        }
        debug!("Runtime has shut down");
        Ok(())
    }

    pub(crate) async fn serve_connection(
        &self,
        stream: IoStream,
        job_sender: async_channel::Sender<RequestJob>,
        listener: Arc<ListenerInfo>,
        shutdown_channel: watch::Receiver<RunningPhase>,
    ) {
        let addr = stream.addr();
        let io: TokioIo<Pin<Box<IoStream>>> = TokioIo::new(Box::pin(stream));
        let executor = self.executor.clone();
        let mut shutdown_channel_clone = shutdown_channel.clone();
        let mut executor = executor.clone();
        let mut binding = executor.http1();
        let shutdown_channel = shutdown_channel_clone.clone();

        let service = ItsiHttpService {
            inner: Arc::new(ItsiHttpServiceInner {
                sender: job_sender.clone(),
                server_params: self.server_config.server_params.read().clone(),
                listener,
                addr: addr.to_string(),
                shutdown_channel: shutdown_channel.clone(),
            }),
        };
        let mut serve = Box::pin(
            binding
                .timer(TokioTimer::new())
                .header_read_timeout(Duration::from_secs(1))
                .serve_connection_with_upgrades(io, service),
        );

        tokio::select! {
            // Await the connection finishing naturally.
            res = &mut serve => {
                match res{
                    Ok(()) => {
                      debug!("Connection closed normally")
                    },
                    Err(res) => {
                      debug!("Connection closed abruptly: {:?}", res)
                    }
                }
                serve.as_mut().graceful_shutdown();
            },
            // A lifecycle event triggers shutdown.
            _ = shutdown_channel_clone.changed() => {
                // Initiate graceful shutdown.
                info!("Starting graceful shutdown");
                serve.as_mut().graceful_shutdown();

                // Now await the connection to finish shutting down.
                if let Err(e) = serve.await {
                    debug!("Connection shutdown error: {:?}", e);
                }
            }
        }
    }

    /// Attempts to reload the config "live"
    /// Not that when running in single mode this will not unload
    /// old code. If you need a clean restart, use the `restart` (SIGHUP) method instead
    pub fn reload(&self) -> Result<()> {
        let should_reexec = self.server_config.clone().reload(false)?;
        if should_reexec {
            self.server_config.dup_fds()?;
            self.server_config.reload_exec()?;
        }
        self.restart_requested.store(true, Ordering::SeqCst);
        self.stop()?;
        self.server_config.server_params.read().preload_ruby()?;
        Ok(())
    }

    /// Restart the server while keeping connections open.
    pub fn restart(&self) -> Result<()> {
        self.server_config.dup_fds()?;
        self.server_config.reload_exec()?;
        Ok(())
    }
}
