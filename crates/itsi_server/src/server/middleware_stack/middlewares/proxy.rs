use std::{
    collections::HashMap,
    convert::Infallible,
    error::Error,
    net::SocketAddr,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, LazyLock, OnceLock,
    },
    time::Duration,
};

use super::{string_rewrite::StringRewrite, ErrorResponse, FromValue, MiddlewareLayer};
use crate::{
    server::{
        binds::bind::{Bind, BindAddress},
        http_message_types::{HttpRequest, HttpResponse, RequestExt, ResponseFormat},
        size_limited_incoming::MaxBodySizeReached,
    },
    services::itsi_http_service::HttpRequestContext,
};
use async_trait::async_trait;
use bytes::{Bytes, BytesMut};
use either::Either;
use futures::TryStreamExt;
use http::{HeaderMap, Method, Response, StatusCode};
use http_body_util::{combinators::BoxBody, BodyExt, Empty, StreamBody};
use hyper::body::Frame;
use magnus::error::Result;
use rand::Rng;
use reqwest::{
    dns::{Name, Resolve},
    Body, Client, Url,
};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Proxy {
    pub to: StringRewrite,
    pub backends: Vec<String>,
    pub backend_priority: BackendPriority,
    pub headers: HashMap<String, Option<ProxiedHeader>>,
    pub verify_ssl: bool,
    pub timeout: u64,
    pub tls_sni: bool,
    #[serde(skip_deserializing)]
    pub client: OnceLock<Client>,
    #[serde(default = "bad_gateway_error_response")]
    pub error_response: ErrorResponse,
}

fn bad_gateway_error_response() -> ErrorResponse {
    ErrorResponse::bad_gateway()
}

#[derive(Debug, Clone, Deserialize)]
pub enum BackendPriority {
    #[serde(rename(deserialize = "round_robin"))]
    RoundRobin,
    #[serde(rename(deserialize = "ordered"))]
    Ordered,
    #[serde(rename(deserialize = "random"))]
    Random,
}

#[derive(Debug, Clone, Deserialize)]
pub enum ProxiedHeader {
    #[serde(rename(deserialize = "value"))]
    String(String),
    #[serde(rename(deserialize = "rewrite"))]
    StringRewrite(StringRewrite),
}

impl ProxiedHeader {
    pub fn to_string(&self, req: &HttpRequest, context: &HttpRequestContext) -> String {
        match self {
            ProxiedHeader::String(value) => value.clone(),
            ProxiedHeader::StringRewrite(rewrite) => rewrite.rewrite_request(req, context),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Resolver {
    backends: Arc<Vec<SocketAddr>>,
    counter: Arc<AtomicUsize>,
    backend_priority: BackendPriority,
}

pub struct StatefulResolverIter {
    backends: Arc<Vec<SocketAddr>>,
    start_index: usize,
    current: usize,
}

impl Iterator for StatefulResolverIter {
    type Item = SocketAddr;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current < self.backends.len() {
            let index = (self.start_index + self.current) % self.backends.len();
            self.current += 1;
            Some(self.backends[index])
        } else {
            None
        }
    }
}

impl Resolve for Resolver {
    fn resolve(&self, _name: Name) -> reqwest::dns::Resolving {
        let backends = self.backends.clone();
        let len = backends.len();

        let start_index = match self.backend_priority {
            BackendPriority::Ordered => 0,
            BackendPriority::Random => rand::rng().random_range(0..len),
            BackendPriority::RoundRobin => self.counter.fetch_add(1, Ordering::Relaxed) % len,
        };

        let fut = async move {
            let iter = StatefulResolverIter {
                backends,
                start_index,
                current: 0,
            };
            Ok(Box::new(iter) as Box<dyn Iterator<Item = SocketAddr> + Send>)
        };

        Box::pin(fut)
    }
}

static BAD_GATEWAY_RESPONSE: LazyLock<ErrorResponse> = LazyLock::new(ErrorResponse::bad_gateway);
static GATEWAY_TIMEOUT_RESPONSE: LazyLock<ErrorResponse> =
    LazyLock::new(ErrorResponse::gateway_timeout);
static SERVICE_UNAVAILABLE_RESPONSE: LazyLock<ErrorResponse> =
    LazyLock::new(ErrorResponse::service_unavailable);
static INTERNAL_SERVER_ERROR_RESPONSE: LazyLock<ErrorResponse> =
    LazyLock::new(ErrorResponse::internal_server_error);

fn is_idempotent(method: &Method) -> bool {
    matches!(
        *method,
        Method::GET | Method::HEAD | Method::PUT | Method::DELETE | Method::OPTIONS
    )
}

/// A helper that stores the immutable parts of the incoming request.
struct RequestInfo {
    method: Method,
    headers: HeaderMap,
}

impl Proxy {
    /// Build a header map of overriding headers based on the configured values.
    /// This uses the full HttpRequest and context to compute each header value.
    fn build_overriding_headers(
        &self,
        req: &HttpRequest,
        context: &mut HttpRequestContext,
    ) -> http::HeaderMap {
        let mut headers = http::HeaderMap::new();
        for (name, header_opt) in self.headers.iter() {
            if let Some(header_value) = header_opt {
                // Compute the header value using the full HttpRequest.
                let value_str = header_value.to_string(req, context);
                if let Ok(header_val) = http::HeaderValue::from_str(&value_str) {
                    if let Ok(header_name) = name.parse::<http::header::HeaderName>() {
                        headers.insert(header_name, header_val);
                    }
                }
            }
        }
        headers
    }

    /// Build a reqwest::RequestBuilder by merging the original request headers
    /// (unless overridden) with the precomputed overriding headers.
    fn build_reqwest_request_info(
        &self,
        req_info: &RequestInfo,
        url: &str,
        host_str: &str,
        body: Body,
        overriding_headers: &http::HeaderMap,
    ) -> reqwest::RequestBuilder {
        let mut builder = self
            .client
            .get()
            .unwrap()
            .request(req_info.method.clone(), url);

        // Forward headers from the original request unless they are overridden.
        for (name, value) in req_info.headers.iter() {
            if overriding_headers.contains_key(name) {
                continue;
            }
            builder = builder.header(name, value);
        }

        // Add a Host header if not overridden.
        if !overriding_headers.contains_key("host") && !host_str.is_empty() {
            builder = builder.header("Host", host_str);
        }

        for (name, value) in overriding_headers.iter() {
            builder = builder.header(name, value);
        }

        builder.body(body)
    }

    /// Sends an idempotent request using a replayable (buffered) body.
    async fn send_request_idempotent(
        &self,
        req_info: &RequestInfo,
        url: &str,
        host_str: &str,
        max_attempts: usize,
        replayable_bytes: Bytes,
        overriding_headers: &http::HeaderMap,
    ) -> std::result::Result<reqwest::Response, reqwest::Error> {
        let mut last_err = None;
        for attempt in 0..max_attempts {
            let body = Body::from(replayable_bytes.clone());
            let builder =
                self.build_reqwest_request_info(req_info, url, host_str, body, overriding_headers);
            match builder.send().await {
                Ok(response) => return Ok(response),
                Err(e) => {
                    // Retry for connectivity-related errors.
                    if e.is_connect() {
                        last_err = Some(e);
                        if attempt + 1 < max_attempts {
                            continue;
                        } else {
                            break;
                        }
                    } else {
                        return Err(e);
                    }
                }
            }
        }
        Err(last_err.expect("At least one attempt should have set last_err"))
    }

    /// Sends a non-idempotent request once using its streaming body.
    async fn send_request_non_idempotent(
        &self,
        req: HttpRequest,
        req_info: &RequestInfo,
        url: &str,
        host_str: &str,
        overriding_headers: &http::HeaderMap,
    ) -> std::result::Result<reqwest::Response, reqwest::Error> {
        let body = Body::wrap_stream(req.into_data_stream());
        let builder =
            self.build_reqwest_request_info(req_info, url, host_str, body, overriding_headers);
        builder.send().await
    }
}

#[async_trait]
impl MiddlewareLayer for Proxy {
    async fn initialize(&self) -> Result<()> {
        let backends = self
            .backends
            .iter()
            .filter_map(|be| {
                let bind: Bind = be.parse().ok()?;
                match (bind.address, bind.port) {
                    (BindAddress::Ip(ip_addr), port) => {
                        Some(SocketAddr::new(ip_addr, port.unwrap()))
                    }
                    (BindAddress::UnixSocket(_), _) => None,
                }
            })
            .collect::<Vec<_>>();

        self.client
            .set(
                Client::builder()
                    .timeout(Duration::from_secs(self.timeout))
                    .danger_accept_invalid_certs(!self.verify_ssl)
                    .danger_accept_invalid_hostnames(!self.verify_ssl)
                    .dns_resolver(Arc::new(Resolver {
                        backends: Arc::new(backends),
                        counter: Arc::new(AtomicUsize::new(0)),
                        backend_priority: self.backend_priority.clone(),
                    }))
                    .tls_sni(self.tls_sni)
                    .build()
                    .map_err(|e| {
                        magnus::Error::new(
                            magnus::exception::runtime_error(),
                            format!("Failed to build Reqwest client: {}", e),
                        )
                    })?,
            )
            .map_err(|_e| {
                magnus::Error::new(
                    magnus::exception::standard_error(),
                    "Failed to save resolver backends",
                )
            })?;
        Ok(())
    }

    async fn before(
        &self,
        req: HttpRequest,
        context: &mut HttpRequestContext,
    ) -> Result<Either<HttpRequest, HttpResponse>> {
        let url = self.to.rewrite_request(&req, context);
        let accept: ResponseFormat = req.accept().into();
        let error_response = self.error_response.to_http_response(accept.clone()).await;

        let destination = match Url::parse(&url) {
            Ok(dest) => dest,
            Err(_) => return Ok(Either::Right(error_response)),
        };

        // Clone the headers before consuming the request.
        let req_headers = req.headers().clone();
        let host_str = destination.host_str().unwrap_or_else(|| {
            req_headers
                .get("Host")
                .and_then(|h| h.to_str().ok())
                .unwrap_or("")
        });

        let req_info = RequestInfo {
            method: req.method().clone(),
            headers: req_headers.clone(),
        };

        // Precompute the overriding headers from the full request.
        let overriding_headers = self.build_overriding_headers(&req, context);

        // Determine max_attempts based on the number of backends.
        let max_attempts = self.backends.len();

        let reqwest_response_result = if is_idempotent(&req_info.method) {
            let (_parts, body) = req.into_parts();
            let replayable_bytes = match body.into_data_stream().try_collect::<Vec<Bytes>>().await {
                Ok(chunks) => {
                    let mut buf = BytesMut::new();
                    for chunk in chunks {
                        buf.extend_from_slice(&chunk);
                    }
                    buf.freeze()
                }
                Err(e) => {
                    tracing::error!("Error buffering request body: {}", e);
                    return Ok(Either::Right(error_response));
                }
            };
            self.send_request_idempotent(
                &req_info,
                &url,
                host_str,
                max_attempts,
                replayable_bytes,
                &overriding_headers,
            )
            .await
        } else {
            self.send_request_non_idempotent(req, &req_info, &url, host_str, &overriding_headers)
                .await
        };

        let response = match reqwest_response_result {
            Ok(response) => {
                let status = response.status();
                let mut builder = Response::builder().status(status);
                for (hn, hv) in response.headers() {
                    builder = builder.header(hn, hv);
                }
                let response = builder.body(BoxBody::new(StreamBody::new(
                    response
                        .bytes_stream()
                        .map_ok(Frame::data)
                        .map_err(|_| -> Infallible { unreachable!("We handle IO errors above") }),
                )));
                response.unwrap_or(error_response)
            }
            Err(e) => {
                if let Some(inner) = e.source() {
                    if inner.downcast_ref::<MaxBodySizeReached>().is_some() {
                        let mut max_body_response = Response::new(BoxBody::new(Empty::new()));
                        *max_body_response.status_mut() = StatusCode::PAYLOAD_TOO_LARGE;
                        return Ok(Either::Right(max_body_response));
                    }
                }
                if e.is_timeout() {
                    GATEWAY_TIMEOUT_RESPONSE.to_http_response(accept).await
                } else if e.is_connect() {
                    BAD_GATEWAY_RESPONSE.to_http_response(accept).await
                } else if e.is_status() {
                    SERVICE_UNAVAILABLE_RESPONSE.to_http_response(accept).await
                } else {
                    INTERNAL_SERVER_ERROR_RESPONSE
                        .to_http_response(accept)
                        .await
                }
            }
        };

        Ok(Either::Right(response))
    }
}
impl FromValue for Proxy {}
