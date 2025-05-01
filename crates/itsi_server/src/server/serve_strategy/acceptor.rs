use std::{ops::Deref, pin::Pin, sync::Arc, time::Duration};

use hyper_util::rt::TokioIo;
use tokio::task::JoinSet;
use tracing::debug;

use crate::{
    ruby_types::itsi_server::itsi_server_config::ServerParams,
    server::{binds::listener::ListenerInfo, io_stream::IoStream, request_job::RequestJob},
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
    pub(crate) async fn serve_connection(&mut self, stream: IoStream) {
        let addr = stream.addr();
        let io: TokioIo<Pin<Box<IoStream>>> = TokioIo::new(Box::pin(stream));
        let mut shutdown_channel = self.shutdown_receiver.clone();
        let acceptor_args = self.acceptor_args.clone();
        self.join_set.spawn(async move {
            let executor = &acceptor_args.strategy.executor;
            let mut serve = Box::pin(executor.serve_connection_with_upgrades(
                io,
                ItsiHttpService {
                    inner: Arc::new(ItsiHttpServiceInner {
                        acceptor_args: acceptor_args.clone(),
                        addr: addr.to_string(),
                    }),
                },
            ));

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
                    serve.as_mut().graceful_shutdown();
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
                debug!("Shutdown timeout reached; abandoning remaining acceptor tasks.");
            }
        }
    }
}
