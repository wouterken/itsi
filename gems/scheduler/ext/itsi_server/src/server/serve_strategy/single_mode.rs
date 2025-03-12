use crate::{
    request::itsi_request::ItsiRequest,
    server::{
        io_stream::IoStream,
        itsi_server::{RequestJob, Server},
        lifecycle_event::LifecycleEvent,
        listener::{Listener, TokioListener},
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
use std::{
    num::NonZeroU8,
    pin::Pin,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    runtime::{Builder as RuntimeBuilder, Runtime},
    sync::broadcast,
    task::JoinSet,
};
use tracing::instrument;

pub struct SingleMode {
    pub executor: Builder<TokioExecutor>,
    pub server: Arc<Server>,
    pub sender: async_channel::Sender<RequestJob>,
    pub(crate) listeners: Arc<Vec<Arc<Listener>>>,
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
        listeners: Arc<Vec<Arc<Listener>>>,
        lifecycle_channel: broadcast::Sender<LifecycleEvent>,
    ) -> Result<Self> {
        let (thread_workers, sender) = build_thread_workers(
            Pid::this(),
            NonZeroU8::try_from(server.threads).unwrap(),
            server.app,
            server.scheduler_class.clone(),
        )?;
        Ok(Self {
            executor: Builder::new(TokioExecutor::new()),
            listeners,
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
        Ok(())
    }

    #[instrument(parent=None, skip(self))]
    pub fn run(self: Arc<Self>) -> Result<()> {
        let mut listener_task_set = JoinSet::new();
        let self_ref = Arc::new(self);
        self_ref.build_runtime().block_on(async {

          for listener in self_ref.listeners.clone().iter() {
              let listener = Arc::new(listener.to_tokio_listener());
              let mut lifecycle_rx = self_ref.lifecycle_channel.subscribe();
              let self_ref = self_ref.clone();
              let listener = listener.clone();
              let (shutdown_sender, mut shutdown_receiver) = tokio::sync::watch::channel::<RunningPhase>(RunningPhase::Running);
              listener_task_set.spawn(async move {
                let strategy = self_ref.clone();
                loop {
                    tokio::select! {
                        accept_result = listener.accept() => match accept_result {
                          Ok(accept_result) => {
                            if let Err(e) = strategy.serve_connection(accept_result, listener.clone(), shutdown_receiver.clone()).await {
                              error!("Error in serve_connection {:?}", e)
                            }
                          },
                          Err(e) => debug!("Listener.accept failed {:?}", e),
                        },
                        _ = shutdown_receiver.changed() => {
                          break;
                        }
                        lifecycle_event = lifecycle_rx.recv() => match lifecycle_event{
                          Ok(lifecycle_event) => {
                            if let Err(e) = strategy.handle_lifecycle_event(lifecycle_event, shutdown_sender.clone()).await{
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
        });

        Ok(())
    }

    pub(crate) async fn serve_connection(
        &self,
        stream: IoStream,
        listener: Arc<TokioListener>,
        shutdown_channel: tokio::sync::watch::Receiver<RunningPhase>,
    ) -> Result<()> {
        let sender_clone = self.sender.clone();
        let addr = stream.addr();
        let io: TokioIo<Pin<Box<IoStream>>> = TokioIo::new(Box::pin(stream));
        let server = self.server.clone();
        let executor = self.executor.clone();
        let mut shutdown_channel_clone = shutdown_channel.clone();
        tokio::spawn(async move {
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
        });
        Ok(())
    }

    pub async fn handle_lifecycle_event(
        &self,
        lifecycle_event: LifecycleEvent,
        shutdown_sender: tokio::sync::watch::Sender<RunningPhase>,
    ) -> Result<()> {
        if let LifecycleEvent::Shutdown = lifecycle_event {
            shutdown_sender
                .send(RunningPhase::ShutdownPending)
                .expect("Failed to send shutdown pending signal");
            let deadline = Instant::now() + Duration::from_secs_f64(self.server.shutdown_timeout);
            for worker in &*self.thread_workers {
                worker.request_shutdown().await;
            }
            while Instant::now() < deadline {
                tokio::time::sleep(Duration::from_millis(50)).await;
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

            info!("Sending shutdown signal");
            shutdown_sender
                .send(RunningPhase::Shutdown)
                .expect("Failed to send shutdown signal");
            self.thread_workers.iter().for_each(|worker| {
                worker.poll_shutdown(deadline);
            });

            return Err(ItsiError::Break());
        }
        Ok(())
    }
}
