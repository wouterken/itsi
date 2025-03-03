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
use futures::future;
use http::Request;
use hyper::{body::Incoming, service::service_fn};
use hyper_util::{
    rt::{TokioExecutor, TokioIo},
    server::conn::auto::Builder,
};
use itsi_error::{ItsiError, Result};
use itsi_tracing::{debug, error, info};
use magnus::{value::Opaque, Value};
use std::{num::NonZeroU8, pin::Pin, sync::Arc};
use tokio::{
    runtime::{Builder as RuntimeBuilder, Runtime},
    task::JoinSet,
};

pub struct SingleMode {
    pub server: Builder<TokioExecutor>,
    pub script_name: String,
    pub sender: Arc<Sender<RequestJob>>,
    pub shutdown_timeout: f64,
    pub(crate) listeners: Arc<Vec<Arc<Listener>>>,
    pub(crate) thread_workers: Arc<Vec<ThreadWorker>>,
}

impl SingleMode {
    pub(crate) fn new(
        app: Opaque<Value>,
        listeners: Arc<Vec<Arc<Listener>>>,
        server: Builder<TokioExecutor>,
        thread_count: NonZeroU8,
        script_name: String,
        shutdown_timeout: f64,
    ) -> Self {
        let (thread_workers, sender) = build_thread_workers(thread_count, app);
        Self {
            server,
            listeners,
            script_name,
            sender,
            shutdown_timeout,
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
              listener_task_set.spawn(async move {
                let strategy = self_ref.clone();
                loop {
                    tokio::select! {
                        accept_result = listener.accept() => match accept_result {
                          Ok(accept_result) => {
                            if let Err(e) = strategy.serve_connection(accept_result, listener.clone()).await {
                              error!("Error in serve_connection {:?}", e)
                            }
                          },
                          Err(e) => error!("Error in listener.accept {:?}", e),
                      },
                        lifecycle_event = lifecycle_rx.recv() => match lifecycle_event{
                          Ok(lifecycle_event) => {
                            if let Err(e) = strategy.handle_lifecycle_event(lifecycle_event).await{
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
    ) -> Result<()> {
        let sender_clone = self.sender.clone();
        let addr = stream.addr();
        let io: TokioIo<Pin<Box<IoStream>>> = TokioIo::new(Box::pin(stream));
        let script_name = self.script_name.clone();
        let server = self.server.clone();
        tokio::spawn(async move {
            if let Err(e) = server
                .serve_connection_with_upgrades(
                    io,
                    service_fn(move |hyper_request: Request<Incoming>| {
                        ItsiRequest::process_request(
                            hyper_request,
                            sender_clone.clone(),
                            script_name.clone(),
                            listener.clone(),
                            addr.clone(),
                        )
                    }),
                )
                .await
            {
                debug!("Closed connection due to: {:?}", e);
            }
        });
        Ok(())
    }

    pub async fn handle_lifecycle_event(&self, lifecycle_event: LifecycleEvent) -> Result<()> {
        if let LifecycleEvent::Shutdown = lifecycle_event {
            info!("Shutdown event received; exiting listener loop.");
            let thread_workers_futures = self
                .thread_workers
                .iter()
                .map(|worker| async move { worker.shutdown(self.shutdown_timeout).await });
            future::join_all(thread_workers_futures).await;
            return Err(ItsiError::Break());
        }
        Ok(())
    }
}
