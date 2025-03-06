use crate::{
    request::itsi_request::ItsiRequest,
    server::{
        io_stream::IoStream,
        itsi_server::RequestJob,
        lifecycle_event::LifecycleEvent,
        listener::{Listener, TokioListener},
        signal::handle_signals,
        thread_worker::{build_thread_workers, ThreadWorker},
    },
};
use crossbeam::channel::Sender;
use http::Request;
use hyper::{body::Incoming, service::service_fn};
use hyper_util::{
    rt::{TokioExecutor, TokioIo, TokioTimer},
    server::conn::auto::Builder,
};
use itsi_error::{ItsiError, Result};
use itsi_tracing::{debug, error, info};
use magnus::{value::Opaque, Value};
use nix::unistd::Pid;
use std::{
    num::NonZeroU8,
    pin::Pin,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    runtime::{Builder as RuntimeBuilder, Runtime},
    task::JoinSet,
};
use tracing::instrument;

pub struct SingleMode {
    pub server: Builder<TokioExecutor>,
    pub script_name: String,
    pub sender: Arc<Sender<RequestJob>>,
    pub shutdown_timeout: f64,
    pub scheduler_class: Option<String>,
    pub(crate) listeners: Arc<Vec<Arc<Listener>>>,
    pub(crate) thread_workers: Arc<Vec<ThreadWorker>>,
}

pub enum RunningPhase {
    Running,
    ShutdownPending,
    Shutdown,
}

impl SingleMode {
    pub(crate) fn new(
        app: Opaque<Value>,
        listeners: Arc<Vec<Arc<Listener>>>,
        thread_count: NonZeroU8,
        script_name: String,
        scheduler_class: Option<String>,
        shutdown_timeout: f64,
    ) -> Self {
        let (thread_workers, sender) =
            build_thread_workers(Pid::this(), thread_count, app, scheduler_class.clone());
        Self {
            server: Builder::new(TokioExecutor::new()),
            listeners,
            script_name,
            sender,
            shutdown_timeout,
            scheduler_class,
            thread_workers,
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

    #[instrument(skip(self), fields(mode = "single"))]
    pub fn run(self: Arc<Self>) -> Result<()> {
        let (lifecycle_tx, _) = tokio::sync::broadcast::channel::<LifecycleEvent>(100);
        let lifecycle_tx = Arc::new(lifecycle_tx);
        let mut listener_task_set = JoinSet::new();

        let self_ref = Arc::new(self);

        self_ref.build_runtime().block_on(async {
          let signals_task = tokio::spawn(handle_signals(lifecycle_tx.clone()));
          for listener in self_ref.listeners.clone().iter() {
              let listener = Arc::new(listener.to_tokio_listener());
              let mut lifecycle_rx = lifecycle_tx.subscribe();
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
          if let Err(e) =  signals_task.await {
              error!("Error closing server: {:?}", e);
          }
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
        let script_name = self.script_name.clone();
        let server = self.server.clone();
        let mut shutdown_channel_clone = shutdown_channel.clone();
        tokio::spawn(async move {
            let mut server = server.clone();
            let mut binding = server.http1();
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
                                script_name.clone(),
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
                    info!("Received shutdown signal in serve_fn");
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
            let deadline = Instant::now() + Duration::from_secs_f64(self.shutdown_timeout);
            self.thread_workers
                .iter()
                .for_each(|worker| worker.request_shutdown());
            while Instant::now() < deadline {
                tokio::time::sleep(Duration::from_millis(200)).await;
                let alive_threads = self
                    .thread_workers
                    .iter()
                    .filter(|worker| {
                        info!("Checking worker status {}", worker.id);
                        worker.poll_shutdown(deadline)
                    })
                    .count();
                if alive_threads == 0 {
                    break;
                }
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
