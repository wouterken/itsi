use super::listener::ListenerInfo;
use super::request_job::RequestJob;
use super::serve_strategy::single_mode::RunningPhase;
use super::types::HttpRequest;
use super::{filter_stack::FilterLayer, types::HttpResponse};
use crate::ruby_types::itsi_server::itsi_server_config::ServerParams;
use either::Either;
use hyper::service::Service;
use itsi_error::ItsiError;
use std::{future::Future, ops::Deref, pin::Pin, sync::Arc};
use tokio::sync::watch::{self};
use tracing::info;

#[derive(Clone)]
pub struct ItsiService {
    pub inner: Arc<IstiServiceInner>,
}

impl Deref for ItsiService {
    type Target = Arc<IstiServiceInner>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

pub struct IstiServiceInner {
    pub sender: async_channel::Sender<RequestJob>,
    pub server_params: Arc<ServerParams>,
    pub listener: Arc<ListenerInfo>,
    pub addr: String,
    pub shutdown_channel: watch::Receiver<RunningPhase>,
}

impl Service<HttpRequest> for ItsiService {
    type Response = HttpResponse;
    type Error = ItsiError;
    type Future = Pin<Box<dyn Future<Output = itsi_error::Result<HttpResponse>> + Send>>;

    // This is called once per incoming Request.
    fn call(&self, req: HttpRequest) -> Self::Future {
        let params = self.server_params.clone();
        let context = self.clone();
        Box::pin(async move {
            let mut req = req;
            let mut resp: Option<HttpResponse> = None;
            let stack = params.middleware.get().unwrap().stack_for(&req);
            for elm in stack.iter() {
                match elm.before(req, &context).await {
                    Ok(Either::Left(r)) => req = r,
                    Ok(Either::Right(r)) => {
                        resp = Some(r);
                        break;
                    }
                    Err(e) => return Err(e.into()),
                }
            }

            let mut resp = match resp {
                Some(r) => r,
                None => {
                    return Err(ItsiError::InternalServerError(
                        "No response returned from middleware stack".to_string(),
                    ))
                }
            };

            for elm in stack.iter() {
                resp = elm.after(resp).await;
            }

            Ok(resp)
        })
    }
}
