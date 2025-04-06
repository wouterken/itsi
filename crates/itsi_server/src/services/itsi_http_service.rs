use crate::ruby_types::itsi_server::itsi_server_config::ServerParams;
use crate::server::binds::listener::ListenerInfo;
use crate::server::http_message_types::{ConversionExt, HttpResponse, RequestExt, ResponseFormat};
use crate::server::middleware_stack::{CompressionAlgorithm, ErrorResponse, MiddlewareLayer};
use crate::server::request_job::RequestJob;
use crate::server::serve_strategy::single_mode::RunningPhase;
use chrono;
use chrono::Local;
use either::Either;
use http::Request;
use hyper::body::Incoming;
use hyper::service::Service;
use itsi_error::ItsiError;
use regex::Regex;
use std::sync::{LazyLock, OnceLock};

use std::{future::Future, ops::Deref, pin::Pin, sync::Arc};
use tokio::sync::watch::{self};
use tokio::time::timeout;

#[derive(Clone)]
pub struct ItsiHttpService {
    pub inner: Arc<ItsiHttpServiceInner>,
}

impl Deref for ItsiHttpService {
    type Target = Arc<ItsiHttpServiceInner>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

pub struct ItsiHttpServiceInner {
    pub sender: async_channel::Sender<RequestJob>,
    pub server_params: Arc<ServerParams>,
    pub listener: Arc<ListenerInfo>,
    pub addr: String,
    pub shutdown_channel: watch::Receiver<RunningPhase>,
}

#[derive(Clone)]
pub struct HttpRequestContext {
    inner: Arc<RequestContextInner>,
}

impl Deref for HttpRequestContext {
    type Target = Arc<RequestContextInner>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl Deref for RequestContextInner {
    type Target = ItsiHttpService;

    fn deref(&self) -> &Self::Target {
        &self.service
    }
}

pub struct RequestContextInner {
    pub request_id: u128,
    pub service: ItsiHttpService,
    pub accept: ResponseFormat,
    pub matching_pattern: Option<Arc<Regex>>,
    pub compression_method: OnceLock<CompressionAlgorithm>,
    pub origin: OnceLock<Option<String>>,
    pub response_format: OnceLock<ResponseFormat>,
    pub start_time: chrono::DateTime<chrono::Utc>,
    pub request: Option<Arc<Request<Incoming>>>,
    pub request_start_time: OnceLock<chrono::DateTime<Local>>,
    pub if_none_match: OnceLock<Option<String>>,
    pub etag_value: OnceLock<Option<String>>,
}

impl HttpRequestContext {
    fn new(
        service: ItsiHttpService,
        matching_pattern: Option<Arc<Regex>>,
        accept: ResponseFormat,
    ) -> Self {
        HttpRequestContext {
            inner: Arc::new(RequestContextInner {
                request_id: rand::random::<u128>(),
                service,
                matching_pattern,
                accept,
                compression_method: OnceLock::new(),
                origin: OnceLock::new(),
                response_format: OnceLock::new(),
                start_time: chrono::Utc::now(),
                request: None,
                request_start_time: OnceLock::new(),
                if_none_match: OnceLock::new(),
                etag_value: OnceLock::new(),
            }),
        }
    }

    pub fn set_compression_method(&self, method: CompressionAlgorithm) {
        self.inner.compression_method.set(method).unwrap();
    }

    pub fn set_origin(&self, origin: Option<String>) {
        self.inner.origin.set(origin).unwrap();
    }

    pub fn set_if_none_match(&self, value: Option<String>) {
        self.inner.if_none_match.set(value).unwrap();
    }

    pub fn get_if_none_match(&self) -> Option<String> {
        self.inner.if_none_match.get().cloned().flatten()
    }

    pub fn short_request_id(&self) -> String {
        format!("{:016x}", self.inner.request_id & 0xffff_ffff_ffff_ffff)
    }

    pub fn request_id(&self) -> String {
        format!("{:016x}", self.inner.request_id)
    }

    pub fn track_start_time(&self) {
        self.inner
            .request_start_time
            .get_or_init(chrono::Local::now);
    }

    pub fn start_time(&self) -> Option<chrono::DateTime<Local>> {
        self.inner.request_start_time.get().cloned()
    }

    pub fn get_response_time(&self) -> Option<chrono::TimeDelta> {
        self.inner
            .request_start_time
            .get()
            .map(|instant| Local::now() - instant)
    }

    pub fn set_response_format(&self, format: ResponseFormat) {
        self.inner.response_format.set(format).unwrap()
    }

    pub fn response_format(&self) -> &ResponseFormat {
        self.inner.response_format.get().unwrap()
    }
}

static TIMEOUT_RESPONSE: LazyLock<ErrorResponse> = LazyLock::new(ErrorResponse::gateway_timeout);
static NOT_FOUND_RESPONSE: LazyLock<ErrorResponse> = LazyLock::new(ErrorResponse::not_found);

impl Service<Request<Incoming>> for ItsiHttpService {
    type Response = HttpResponse;
    type Error = ItsiError;
    type Future = Pin<Box<dyn Future<Output = itsi_error::Result<HttpResponse>> + Send>>;

    fn call(&self, req: Request<Incoming>) -> Self::Future {
        let params = self.server_params.clone();
        let self_clone = self.clone();
        let mut req = req.limit();
        let accept: ResponseFormat = req.accept().into();
        let accept_clone = accept.clone();
        let request_timeout = self.server_params.request_timeout;
        let service_future = async move {
            let mut resp: Option<HttpResponse> = None;
            let (stack, matching_pattern) = params.middleware.get().unwrap().stack_for(&req)?;
            let mut context =
                HttpRequestContext::new(self_clone, matching_pattern, accept_clone.clone());
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
                None => return Ok(NOT_FOUND_RESPONSE.to_http_response(accept_clone).await),
            };

            for elm in stack.iter().rev().skip(stack.len() - depth - 1) {
                resp = elm.after(resp, &mut context).await;
            }

            Ok(resp)
        };

        Box::pin(async move {
            if let Some(timeout_duration) = request_timeout {
                match timeout(timeout_duration, service_future).await {
                    Ok(result) => result,
                    Err(_) => Ok(TIMEOUT_RESPONSE.to_http_response(accept).await),
                }
            } else {
                service_future.await
            }
        })
    }
}
