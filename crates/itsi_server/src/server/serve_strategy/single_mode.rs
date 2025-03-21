use crate::{
    ruby_types::itsi_server::itsi_server_config::ItsiServerConfig,
    server::{
        io_stream::IoStream,
        itsi_service::{IstiServiceInner, ItsiService},
        lifecycle_event::LifecycleEvent,
        listener::ListenerInfo,
        request_job::RequestJob,
        signal::SIGNAL_HANDLER_CHANNEL,
        thread_worker::{build_thread_workers, ThreadWorker},
    },
};
use hyper_util::{
    rt::{TokioExecutor, TokioIo, TokioTimer},
    server::conn::auto::Builder,
};
use itsi_error::{ItsiError, Result};
use itsi_rb_helpers::print_rb_backtrace;
use itsi_tracing::{debug, error, info};
use nix::unistd::Pid;
use std::{
    num::NonZeroU8,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio::{
    runtime::{Builder as RuntimeBuilder, Runtime},
    sync::{
        broadcast,
        watch::{self, Sender},
    },
    task::JoinSet,
};
use tracing::instrument;

pub struct SingleMode {
    pub executor: Builder<TokioExecutor>,
    pub server_config: Arc<ItsiServerConfig>,
    pub(crate) lifecycle_channel: broadcast::Sender<LifecycleEvent>,
    pub restart_requested: AtomicBool,
}

pub enum RunningPhase {
    Running,
    ShutdownPending,
    Shutdown,
}

impl SingleMode {
    #[instrument(parent=None, skip_all, fields(pid=format!("{:?}", Pid::this())))]
    pub fn new(server_config: Arc<ItsiServerConfig>) -> Result<Self> {
        let server_params = server_config.server_params.read().clone();
        server_params.preload_ruby()?;

        Ok(Self {
            executor: Builder::new(TokioExecutor::new()),
            server_config,
            lifecycle_channel: SIGNAL_HANDLER_CHANNEL.0.clone(),
            restart_requested: AtomicBool::new(false),
        })
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

    pub fn stop(&self) -> Result<()> {
        self.lifecycle_channel.send(LifecycleEvent::Shutdown).ok();
        Ok(())
    }

    #[instrument(parent=None, skip(self), fields(pid=format!("{}", Pid::this())))]
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

        runtime.block_on(async {
              let tokio_listeners = self
                  .server_config.server_params.write().listeners.lock()
                  .drain(..)
                  .map(|list| {
                    Arc::new(list.into_tokio_listener())
                  })
                  .collect::<Vec<_>>();
              let (shutdown_sender, _) = watch::channel(RunningPhase::Running);
              for listener in tokio_listeners.iter() {
                  let mut lifecycle_rx = self.lifecycle_channel.subscribe();
                  let listener_info = Arc::new(listener.listener_info());
                  let self_ref = self.clone();
                  let listener = listener.clone();
                  let shutdown_sender = shutdown_sender.clone();
                  let thread_workers = thread_workers.clone();
                  let job_sender = job_sender.clone();

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
                              Ok(lifecycle_event) => {
                                if let Err(e) = self_ref.handle_lifecycle_event(lifecycle_event, thread_workers.clone(), shutdown_sender.clone()).await{
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
                    while let Some(_res) = acceptor_task_set.join_next().await {}
                });

              }

              while let Some(_res) = listener_task_set.join_next().await {}

            });
        runtime.shutdown_timeout(Duration::from_millis(100));

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

        let service = ItsiService {
            inner: Arc::new(IstiServiceInner {
                sender: job_sender.clone(),
                server_params: self.server_config.server_params.read().clone(),
                listener,
                addr: addr.to_string(),
                shutdown_channel: shutdown_channel.clone(),
            }),
        };
        let mut serve = Box::pin(
            binding
                .timer(TokioTimer::new()) // your existing timer
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
    /// old code. If you need a clean restart, use the `restart` (SIGUSR2) method instead
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

    pub async fn handle_lifecycle_event(
        &self,
        lifecycle_event: LifecycleEvent,
        thread_workers: Arc<Vec<ThreadWorker>>,
        shutdown_sender: Sender<RunningPhase>,
    ) -> Result<()> {
        info!("Handling lifecycle event: {:?}", lifecycle_event);
        match lifecycle_event {
            LifecycleEvent::Restart => {
                self.restart()?;
            }
            LifecycleEvent::Reload => {
                self.reload()?;
            }
            LifecycleEvent::Shutdown => {
                //1. Stop accepting new connections.
                shutdown_sender.send(RunningPhase::ShutdownPending).ok();
                tokio::time::sleep(Duration::from_millis(25)).await;

                //2. Break out of work queues.
                for worker in thread_workers.iter() {
                    worker.request_shutdown().await;
                }

                tokio::time::sleep(Duration::from_millis(25)).await;

                //3. Wait for all threads to finish.
                let deadline = Instant::now()
                    + Duration::from_secs_f64(
                        self.server_config.server_params.read().shutdown_timeout,
                    );
                while Instant::now() < deadline {
                    let alive_threads = &thread_workers
                        .iter()
                        .filter(|worker| worker.poll_shutdown(deadline))
                        .count();
                    if *alive_threads == 0 {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(200)).await;
                }

                //4. Force shutdown any stragglers
                shutdown_sender.send(RunningPhase::Shutdown).ok();
                thread_workers.iter().for_each(|worker| {
                    worker.poll_shutdown(deadline);
                });

                return Err(ItsiError::Break());
            }
            _ => {}
        }
        Ok(())
    }
}
