use crate::{
    request::itsi_request::ItsiRequest,
    server::{
        io_stream::IoStream,
        itsi_server::{RequestJob, Server},
        lifecycle_event::LifecycleEvent,
        listener::{Listener, ListenerInfo},
        thread_worker::{build_thread_workers, ThreadWorker},
    },
};
use http::Request;
use hyper::{body::Incoming, service::service_fn};
use hyper_util::{
    rt::{TokioExecutor, TokioIo, TokioTimer},
    server::conn::auto::Builder,
};
use itsi_error::{ItsiError, Result};
use itsi_tracing::{debug, error, info};
use nix::unistd::Pid;
use parking_lot::Mutex;
use std::{
    num::NonZeroU8,
    panic,
    pin::Pin,
    sync::Arc,
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
    pub server: Arc<Server>,
    pub sender: async_channel::Sender<RequestJob>,
    pub(crate) listeners: Mutex<Vec<Listener>>,
    pub(crate) thread_workers: Arc<Vec<ThreadWorker>>,
    pub(crate) lifecycle_channel: broadcast::Sender<LifecycleEvent>,
}

pub enum RunningPhase {
    Running,
    ShutdownPending,
    Shutdown,
}

impl SingleMode {
    #[instrument(parent=None, skip_all, fields(pid=format!("{:?}", Pid::this())))]
    pub(crate) fn new(
        server: Arc<Server>,
        listeners: Vec<Listener>,
        lifecycle_channel: broadcast::Sender<LifecycleEvent>,
    ) -> Result<Self> {
        let (thread_workers, sender) = build_thread_workers(
            Pid::this(),
            NonZeroU8::try_from(server.threads).unwrap(),
            server.app.clone(),
            server.scheduler_class.clone(),
        )?;
        Ok(Self {
            executor: Builder::new(TokioExecutor::new()),
            listeners: Mutex::new(listeners),
            server,
            sender,
            thread_workers,
            lifecycle_channel,
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

        runtime.block_on(async {
              let tokio_listeners = self
                  .listeners.lock()
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
                                acceptor_task_set.spawn(async move {
                                  strategy.serve_connection(accept_result, listener_info, shutdown_receiver).await;
                                });
                              },
                              Err(e) => debug!("Listener.accept failed {:?}", e),
                            },
                            _ = shutdown_receiver.changed() => {
                              break;
                            }
                            lifecycle_event = lifecycle_rx.recv() => match lifecycle_event{
                              Ok(lifecycle_event) => {
                                if let Err(e) = self_ref.handle_lifecycle_event(lifecycle_event, shutdown_sender.clone()).await{
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
        debug!("Runtime has shut down");
        Ok(())
    }

    pub(crate) async fn serve_connection(
        &self,
        stream: IoStream,
        listener: Arc<ListenerInfo>,
        shutdown_channel: watch::Receiver<RunningPhase>,
    ) {
        let sender_clone = self.sender.clone();
        let addr = stream.addr();
        let io: TokioIo<Pin<Box<IoStream>>> = TokioIo::new(Box::pin(stream));
        let server = self.server.clone();
        let executor = self.executor.clone();
        let mut shutdown_channel_clone = shutdown_channel.clone();
        let server = server.clone();
        let mut executor = executor.clone();
        let mut binding = executor.http1();
        let shutdown_channel = shutdown_channel_clone.clone();
        let mut serve = Box::pin(
            binding
                .timer(TokioTimer::new())
                .header_read_timeout(Duration::from_secs(1))
                .serve_connection_with_upgrades(
                    io,
                    service_fn(move |hyper_request: Request<Incoming>| {
                        ItsiRequest::process_request(
                            hyper_request,
                            sender_clone.clone(),
                            server.clone(),
                            listener.clone(),
                            addr.clone(),
                            shutdown_channel.clone(),
                        )
                    }),
                ),
        );

        tokio::select! {
            // Await the connection finishing naturally.
            res = &mut serve => {
                match res{
                    Ok(()) => {
                      debug!("Connection closed normally")
                    },
                    Err(res) => {
                      debug!("Connection finished with error: {:?}", res)
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

    pub async fn handle_lifecycle_event(
        &self,
        lifecycle_event: LifecycleEvent,
        shutdown_sender: Sender<RunningPhase>,
    ) -> Result<()> {
        info!("Handling lifecycle event: {:?}", lifecycle_event);
        if let LifecycleEvent::Shutdown = lifecycle_event {
            //1. Stop accepting new connections.
            shutdown_sender.send(RunningPhase::ShutdownPending).ok();
            tokio::time::sleep(Duration::from_millis(50)).await;

            //2. Break out of work queues.
            for worker in &*self.thread_workers {
                worker.request_shutdown().await;
            }

            //3. Wait for all threads to finish.
            let deadline = Instant::now() + Duration::from_secs_f64(self.server.shutdown_timeout);
            while Instant::now() < deadline {
                let alive_threads = self
                    .thread_workers
                    .iter()
                    .filter(|worker| worker.poll_shutdown(deadline))
                    .count();
                if alive_threads == 0 {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(200)).await;
            }

            //4. Force shutdown any stragglers
            shutdown_sender.send(RunningPhase::Shutdown).ok();
            self.thread_workers.iter().for_each(|worker| {
                worker.poll_shutdown(deadline);
            });

            return Err(ItsiError::Break());
        }
        Ok(())
    }
}
