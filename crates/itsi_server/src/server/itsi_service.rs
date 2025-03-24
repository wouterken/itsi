use super::listener::ListenerInfo;
use super::middleware_stack::CompressionAlgorithm;
use super::middleware_stack::MiddlewareLayer;
use super::request_job::RequestJob;
use super::serve_strategy::single_mode::RunningPhase;
use super::types::HttpRequest;
use super::types::HttpResponse;
use crate::ruby_types::itsi_server::itsi_server_config::ServerParams;
use either::Either;
use hyper::service::Service;
use itsi_error::ItsiError;
use regex::Regex;
use std::sync::OnceLock;
use std::{future::Future, ops::Deref, pin::Pin, sync::Arc};
use tokio::sync::watch::{self};

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

#[derive(Clone)]
pub struct RequestContext {
    inner: Arc<RequestContextInner>,
}

impl Deref for RequestContext {
    type Target = Arc<RequestContextInner>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl Deref for RequestContextInner {
    type Target = ItsiService;

    fn deref(&self) -> &Self::Target {
        &self.service
    }
}

pub struct RequestContextInner {
    pub service: ItsiService,
    pub matching_pattern: Option<Arc<Regex>>,
    pub compression_method: OnceLock<CompressionAlgorithm>,
    pub origin: OnceLock<Option<String>>,
}

impl RequestContext {
    fn new(service: ItsiService, matching_pattern: Option<Arc<Regex>>) -> Self {
        RequestContext {
            inner: Arc::new(RequestContextInner {
                service,
                matching_pattern,
                compression_method: OnceLock::new(),
                origin: OnceLock::new(),
            }),
        }
    }

    pub fn set_compression_method(&self, method: CompressionAlgorithm) {
        self.inner.compression_method.set(method).unwrap();
    }

    pub fn set_origin(&self, origin: Option<String>) {
        self.inner.origin.set(origin).unwrap();
    }
}

impl Service<HttpRequest> for ItsiService {
    type Response = HttpResponse;
    type Error = ItsiError;
    type Future = Pin<Box<dyn Future<Output = itsi_error::Result<HttpResponse>> + Send>>;

    // This is called once per incoming Request.
    fn call(&self, req: HttpRequest) -> Self::Future {
        let params = self.server_params.clone();
        let self_clone = self.clone();
        Box::pin(async move {
            let mut req = req;
            let mut resp: Option<HttpResponse> = None;
            let (stack, matching_pattern) = params.middleware.get().unwrap().stack_for(&req);
            let mut context = RequestContext::new(self_clone, matching_pattern);
            let mut depth = 0;
            for (index, elm) in stack.iter().enumerate() {
                match elm.before(req, &mut context).await {
                    Ok(Either::Left(r)) => req = r,
                    Ok(Either::Right(r)) => {
                        resp = Some(r);
                        depth = index;
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

            for elm in stack.iter().rev().skip(stack.len() - depth) {
                resp = elm.after(resp, &mut context).await;
            }

            Ok(resp)
        })
    }
}
