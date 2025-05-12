use crate::default_responses::{NOT_FOUND_RESPONSE, TIMEOUT_RESPONSE};
use crate::ruby_types::itsi_server::itsi_server_config::ItsiServerTokenPreference;
use crate::server::http_message_types::{
    ConversionExt, HttpRequest, HttpResponse, RequestExt, ResponseFormat,
};
use crate::server::lifecycle_event::LifecycleEvent;
use crate::server::middleware_stack::MiddlewareLayer;
use crate::server::serve_strategy::acceptor::AcceptorArgs;
use crate::server::signal::send_lifecycle_event;
use chrono::{self, DateTime, Local};
use either::Either;
use http::header::ACCEPT_ENCODING;
use http::{HeaderValue, Request};
use hyper::body::Incoming;
use regex::Regex;
use smallvec::SmallVec;
use std::ops::Deref;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};
use tokio::time::timeout;
use tracing::error;

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
    pub acceptor_args: Arc<AcceptorArgs>,
    pub addr: String,
}

impl Deref for ItsiHttpServiceInner {
    type Target = Arc<AcceptorArgs>;

    fn deref(&self) -> &Self::Target {
        &self.acceptor_args
    }
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
    pub request_id: u64,
    pub service: ItsiHttpService,
    pub accept: ResponseFormat,
    pub matching_pattern: Option<Arc<Regex>>,
    pub origin: OnceLock<Option<String>>,
    pub response_format: OnceLock<ResponseFormat>,
    pub request_start_time: OnceLock<DateTime<Local>>,
    pub start_instant: Instant,
    pub if_none_match: OnceLock<Option<String>>,
    pub supported_encoding_set: OnceLock<AcceptEncodingSet>,
    pub is_ruby_request: Arc<AtomicBool>,
}

type AcceptEncodingSet = SmallVec<[HeaderValue; 2]>;

impl HttpRequestContext {
    pub fn new(
        service: ItsiHttpService,
        matching_pattern: Option<Arc<Regex>>,
        accept: ResponseFormat,
        is_ruby_request: Arc<AtomicBool>,
    ) -> Self {
        HttpRequestContext {
            inner: Arc::new(RequestContextInner {
                request_id: rand::random::<u64>(),
                service,
                matching_pattern,
                accept,
                origin: OnceLock::new(),
                response_format: OnceLock::new(),
                request_start_time: OnceLock::new(),
                start_instant: Instant::now(),
                if_none_match: OnceLock::new(),
                supported_encoding_set: OnceLock::new(),
                is_ruby_request,
            }),
        }
    }

    pub fn set_supported_encoding_set(&self, req: &HttpRequest) {
        self.inner.supported_encoding_set.get_or_init(|| {
            let mut set: AcceptEncodingSet = SmallVec::new();

            for hv in req.headers().get_all(ACCEPT_ENCODING) {
                set.push(hv.clone()); // clone ≈ 16 B struct copy
            }

            set
        });
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
        format!("{:08x}", self.inner.request_id & 0xffff_ffff)
    }

    pub fn request_id(&self) -> String {
        format!("{:08x}", self.inner.request_id)
    }

    pub fn init_logging_params(&self) {
        self.inner
            .request_start_time
            .get_or_init(chrono::Local::now);
    }

    pub fn start_instant(&self) -> Instant {
        self.inner.start_instant
    }

    pub fn start_time(&self) -> Option<DateTime<Local>> {
        self.inner.request_start_time.get().cloned()
    }

    pub fn get_response_time(&self) -> Duration {
        self.inner.start_instant.elapsed()
    }

    pub fn set_response_format(&self, format: ResponseFormat) {
        self.inner.response_format.set(format).unwrap()
    }

    pub fn response_format(&self) -> &ResponseFormat {
        self.inner.response_format.get().unwrap()
    }

    pub fn supported_encoding_set(&self) -> Option<&AcceptEncodingSet> {
        self.inner.supported_encoding_set.get()
    }
}

const SERVER_TOKEN_VERSION: HeaderValue =
    HeaderValue::from_static(concat!("Itsi/", env!("CARGO_PKG_VERSION")));
const SERVER_TOKEN_NAME: HeaderValue = HeaderValue::from_static("Itsi");

impl ItsiHttpService {
    pub async fn handle_request(&self, req: Request<Incoming>) -> itsi_error::Result<HttpResponse> {
        let mut req = req.limit();
        let accept: ResponseFormat = req.accept().into();
        let is_single_mode = self.server_params.workers == 1;

        let request_timeout = self.server_params.request_timeout;
        let is_ruby_request = Arc::new(AtomicBool::new(false));
        let irr_clone = is_ruby_request.clone();

        let token_preference = self.server_params.itsi_server_token_preference;

        let service_future = async move {
            let middleware_stack = self
                .server_params
                .middleware
                .get()
                .unwrap()
                .stack_for(&req)
                .unwrap();
            let (stack, matching_pattern) = middleware_stack;
            let mut resp: Option<HttpResponse> = None;

            let mut context =
                HttpRequestContext::new(self.clone(), matching_pattern, accept, irr_clone);
            let mut depth = 0;

            for (index, elm) in stack.iter().enumerate() {
                match elm.before(req, &mut context).await {
                    Ok(Either::Left(r)) => req = r,
                    Ok(Either::Right(r)) => {
                        resp = Some(r);
                        depth = index;
                        break;
                    }
                    Err(e) => {
                        error!("Middleware error: {}", e);
                        break;
                    }
                }
            }

            let mut resp = match resp {
                Some(r) => r,
                None => return Ok(NOT_FOUND_RESPONSE.to_http_response(accept).await),
            };

            for elm in stack.iter().rev().skip(stack.len() - depth - 1) {
                resp = elm.after(resp, &mut context).await;
            }

            match token_preference {
                ItsiServerTokenPreference::Version => {
                    resp.headers_mut().insert("Server", SERVER_TOKEN_VERSION);
                }
                ItsiServerTokenPreference::Name => {
                    resp.headers_mut().insert("Server", SERVER_TOKEN_NAME);
                }
                ItsiServerTokenPreference::None => {}
            }

            Ok(resp)
        };

        if let Some(timeout_duration) = request_timeout {
            match timeout(timeout_duration, service_future).await {
                Ok(result) => result,
                Err(_) => {
                    // If we're still running Ruby at this point, we can't just kill the
                    // thread as it might be in a critical section.
                    // Instead we must ask the worker to hot restart.
                    if is_ruby_request.load(Ordering::Relaxed) {
                        if is_single_mode {
                            // If we're in single mode, re-exec the whole process
                            send_lifecycle_event(LifecycleEvent::Restart);
                        } else {
                            // Otherwise we can shutdown the worker and rely on the master to restart it
                            send_lifecycle_event(LifecycleEvent::Shutdown);
                        }
                    }
                    Ok(TIMEOUT_RESPONSE.to_http_response(accept).await)
                }
            }
        } else {
            service_future.await
        }
    }
}
