use std::{pin::Pin, sync::Arc};

use crossbeam::channel::Sender;
use futures::future;
use http::Request;
use hyper::{body::Incoming, service::service_fn};
use hyper_util::{
    rt::{TokioExecutor, TokioIo},
    server::conn::auto::Builder,
};
use itsi_error::{ItsiError, Result};
use itsi_tracing::{debug, info};

use crate::request::itsi_request::ItsiRequest;

use super::{
    itsi_server::RequestJob,
    lifecycle_event::LifecycleEvent,
    listener::{IoStream, Listener, SockAddr},
};

pub enum ServeStrategy {
    Single(Arc<SingleMode>),
    Cluster(Arc<ClusterMode>),
}

pub struct SingleMode {
    pub server: Builder<TokioExecutor>,
    pub script_name: String,
    pub sender: Arc<Sender<RequestJob>>,
    pub shutdown_timeout: f64,
    pub(crate) workers: Arc<Vec<super::thread_worker::ThreadWorker>>,
}

impl SingleMode {
    fn serve_connection(
        self: &Arc<SingleMode>,
        (stream, addr): (TokioIo<Pin<Box<dyn IoStream>>>, SockAddr),
        listener: Arc<Listener>,
    ) -> Result<()> {
        let self_ref = self.clone();
        let sender_clone = self.sender.clone();
        tokio::spawn(async move {
            let script_name = self_ref.script_name.clone();
            if let Err(e) = self_ref
                .server
                .serve_connection_with_upgrades(
                    stream,
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

    async fn handle_lifecycle_event(&self, lifecycle_event: LifecycleEvent) -> Result<()> {
        if let LifecycleEvent::Shutdown = lifecycle_event {
            info!("Shutdown event received; exiting listener loop.");
            let workers_futures = self
                .workers
                .iter()
                .map(|worker| async move { worker.shutdown(self.shutdown_timeout).await });
            future::join_all(workers_futures).await;
            return Err(ItsiError::Break());
        }
        Ok(())
    }
}

pub struct ClusterMode {
    pub server: Builder<TokioExecutor>,
    pub script_name: String,
    pub sender: Arc<Sender<RequestJob>>,
}
impl ClusterMode {
    fn serve_connection(
        &self,
        (_stream, _addr): (TokioIo<Pin<Box<dyn IoStream>>>, SockAddr),
        _listener: Arc<Listener>,
    ) -> Result<()> {
        todo!()
    }

    fn handle_lifecycle_event(&self, _lifecycle_event: LifecycleEvent) -> Result<()> {
        todo!()
    }
}

impl ServeStrategy {
    pub(crate) fn serve_connection(
        &self,
        accept_result: (TokioIo<Pin<Box<dyn IoStream>>>, SockAddr),
        listener: Arc<Listener>,
    ) -> Result<()> {
        match self {
            ServeStrategy::Single(single_router) => {
                single_router.serve_connection(accept_result, listener)
            }
            ServeStrategy::Cluster(cluster_router) => {
                cluster_router.serve_connection(accept_result, listener)
            }
        }
    }

    pub async fn handle_lifecycle_event(&self, lifecycle_event: LifecycleEvent) -> Result<()> {
        match self {
            ServeStrategy::Single(single_router) => {
                single_router.handle_lifecycle_event(lifecycle_event).await
            }
            ServeStrategy::Cluster(cluster_router) => {
                cluster_router.handle_lifecycle_event(lifecycle_event)
            }
        }
    }
}
