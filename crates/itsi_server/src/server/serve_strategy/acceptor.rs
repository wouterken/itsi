use hyper_util::rt::TokioIo;
use std::{future::Future, ops::Deref, pin::Pin, sync::Arc, time::Duration};
use tokio::task::JoinSet;
use tracing::debug;

use crate::{
    ruby_types::itsi_server::itsi_server_config::ServerParams,
    server::{
        binds::listener::{AcceptedStream, ListenerInfo},
        io_stream::IoStream,
        request_job::RequestJob,
    },
    services::itsi_http_service::{ItsiHttpService, ItsiHttpServiceInner},
};

use super::single_mode::{RunningPhase, SingleMode};

pub struct Acceptor {
    pub acceptor_args: Arc<AcceptorArgs>,
    pub join_set: JoinSet<()>,
}

impl Deref for Acceptor {
    type Target = Arc<AcceptorArgs>;

    fn deref(&self) -> &Self::Target {
        &self.acceptor_args
    }
}

pub struct AcceptorArgs {
    pub strategy: Arc<SingleMode>,
    pub listener_info: ListenerInfo,
    pub shutdown_receiver: tokio::sync::watch::Receiver<RunningPhase>,
    pub job_sender: async_channel::Sender<RequestJob>,
    pub nonblocking_sender: async_channel::Sender<RequestJob>,
    pub server_params: Arc<ServerParams>,
}

impl Acceptor {
    pub(crate) async fn serve_accepted_connection(
        &mut self,
        stream: AcceptedStream,
        tls_handshake_timeout: Duration,
    ) {
        self.spawn_connection(async move { stream.into_io_stream(tls_handshake_timeout).await });
    }

    fn spawn_connection<F>(&mut self, stream_future: F)
    where
        F: Future<Output = itsi_error::Result<IoStream>> + Send + 'static,
    {
        let mut shutdown_channel = self.shutdown_receiver.clone();
        let acceptor_args = self.acceptor_args.clone();

        self.join_set.spawn(async move {
            let stream = match stream_future.await {
                Ok(stream) => stream,
                Err(error) => {
                    debug!("Connection setup failed: {:?}", error);
                    return;
                }
            };

            let addr = stream.addr();
            let io: TokioIo<Pin<Box<IoStream>>> = TokioIo::new(Box::pin(stream));
            let service = ItsiHttpService {
                inner: Arc::new(ItsiHttpServiceInner {
                    acceptor_args: acceptor_args.clone(),
                    addr,
                }),
            };

            let executor = &acceptor_args.strategy.executor;
            let svc = hyper::service::service_fn(move |req| {
                let service = service.clone();
                async move { service.handle_request(req).await }
            });

            let mut serve = Box::pin(executor.serve_connection_with_upgrades(io, svc));

            tokio::select! {
                // Await the connection finishing naturally.
                res = &mut serve => {
                    match res {
                        Ok(()) => {
                            debug!("Connection closed normally");
                        },
                        Err(res) => {
                            debug!("Connection closed abruptly: {:?}", res);
                        }
                    }
                },
                // A lifecycle event triggers shutdown.
                _ = shutdown_channel.changed() => {
                    // Initiate graceful shutdown.
                    serve.as_mut().graceful_shutdown();

                    // Now await the connection to finish shutting down.
                    if let Err(e) = serve.await {
                        debug!("Connection shutdown error: {:?}", e);
                    }
                }
            }
        });
    }

    pub async fn join(&mut self) {
        // Join all acceptor tasks with timeout

        let deadline = tokio::time::Instant::now()
            + Duration::from_secs_f64(self.server_params.shutdown_timeout);
        let sleep_until = tokio::time::sleep_until(deadline);
        tokio::select! {
            _ = async {
                while (self.join_set.join_next().await).is_some() {}
            } => {},
            _ = sleep_until => {
                self.join_set.abort_all();
                debug!("Shutdown timeout reached; abandoning remaining acceptor tasks.");
            }
        }
    }
}
