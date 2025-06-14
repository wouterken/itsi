use derive_more::Debug;
use futures::StreamExt;
use http::{header::CONTENT_LENGTH, request::Parts, HeaderValue, Response, StatusCode, Version};
use http_body_util::BodyExt;
use itsi_error::CLIENT_CONNECTION_CLOSED;
use itsi_rb_helpers::{funcall_no_ret, print_rb_backtrace, HeapValue};
use itsi_tracing::debug;
use magnus::{
    block::{yield_values, Proc},
    error::{ErrorType, Result as MagnusResult},
    Error, IntoValue, RHash, Symbol,
};
use magnus::{
    value::{LazyId, ReprValue},
    Ruby, Value,
};
use std::{fmt, io::Write, sync::Arc, time::Instant};
use tracing::error;

use super::{
    itsi_body_proxy::{big_bytes::BigBytes, ItsiBody, ItsiBodyProxy},
    itsi_http_response::{ItsiHttpResponse, ResponseFrame},
};
use crate::{
    default_responses::{INTERNAL_SERVER_ERROR_RESPONSE, SERVICE_UNAVAILABLE_RESPONSE},
    server::{
        http_message_types::{HttpBody, HttpRequest, HttpResponse},
        request_job::RequestJob,
        size_limited_incoming::MaxBodySizeReached,
    },
    services::itsi_http_service::HttpRequestContext,
};

static ID_MESSAGE: LazyId = LazyId::new("message");
static ID_CALL: LazyId = LazyId::new("call");
static ZERO_HEADER_VALUE: HeaderValue = HeaderValue::from_static("0");

#[derive(Debug)]
#[magnus::wrap(class = "Itsi::HttpRequest", free_immediately, size)]
pub struct ItsiHttpRequest {
    pub parts: Arc<Parts>,
    #[debug(skip)]
    pub body: ItsiBody,
    pub version: Version,
    pub response: ItsiHttpResponse,
    pub start: Instant,
    #[debug(skip)]
    pub context: HttpRequestContext,
    pub script_name: String,
}

impl fmt::Display for ItsiHttpRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} {}",
            self.version().unwrap(),
            self.method().unwrap(),
            self.path().unwrap()
        )
    }
}

impl ItsiHttpRequest {
    pub fn is_connection_closed_err(ruby: &Ruby, err: &Error) -> bool {
        match err.error_type() {
            ErrorType::Jump(_) => false,
            ErrorType::Error(_, _) => false,
            ErrorType::Exception(exception) => {
                exception.is_kind_of(ruby.exception_eof_error())
                    && err
                        .value()
                        .map(|v| {
                            v.funcall::<_, _, String>(*ID_MESSAGE, ())
                                .unwrap_or("".to_string())
                                .eq(CLIENT_CONNECTION_CLOSED)
                        })
                        .unwrap_or(false)
            }
        }
    }
    pub fn content_type_str(&self) -> &str {
        self.parts
            .headers
            .get("Content-Type")
            .and_then(|hv| hv.to_str().ok())
            .unwrap_or("application/x-www-form-urlencoded")
    }

    pub fn is_json(&self) -> bool {
        self.content_type_str() == "application/json"
    }

    #[allow(unexpected_cfgs)]
    pub fn url_params(&self) -> magnus::error::Result<RHash> {
        let captures = self
            .context
            .matching_pattern
            .as_ref()
            .and_then(|re| re.captures(self.parts.uri.path()));
        if let Some(caps) = &captures {
            let re = self.context.matching_pattern.as_ref().unwrap();
            let params = {
                // when building against Ruby ≥ 3.2...
                #[cfg(ruby_gte_3_2)]
                {
                    RHash::with_capacity(caps.len())
                }

                // when building against Ruby < 3.2...
                #[cfg(not(ruby_gte_3_2))]
                {
                    RHash::new()
                }
            };
            for (i, group_name) in re.capture_names().enumerate().skip(1) {
                if let Some(name) = group_name {
                    if let Some(m) = caps.get(i) {
                        // Insert into the hash: key is the group name, value is the match.
                        params.aset(Symbol::new(name), m.as_str())?;
                    }
                }
            }
            Ok(params)
        } else {
            Ok(RHash::new())
        }
    }

    pub fn is_html(&self) -> bool {
        self.content_type_str() == "text/html"
    }

    pub fn is_url_encoded(&self) -> bool {
        self.content_type_str() == "application/x-www-form-urlencoded"
    }

    pub fn is_multipart(&self) -> bool {
        self.content_type_str().starts_with("multipart/form-data")
    }

    pub fn content_length(&self) -> Option<u64> {
        self.parts
            .headers
            .get(CONTENT_LENGTH)
            .and_then(|hv| hv.to_str().ok())
            .and_then(|s| s.parse().ok())
    }

    pub fn process(self, ruby: &Ruby, app_proc: Arc<HeapValue<Proc>>) -> magnus::error::Result<()> {
        let response = self.response.clone();

        if let Err(err) =
            funcall_no_ret(app_proc.as_value(), *ID_CALL, [self.into_value_with(ruby)])
        {
            Self::internal_error(ruby, response, err);
        }
        Ok(())
    }

    pub fn internal_error(ruby: &Ruby, response: ItsiHttpResponse, err: Error) {
        if Self::is_connection_closed_err(ruby, &err) {
            debug!("Connection closed by client");
            response.close().ok();
        } else if let Some(rb_err) = err.value() {
            print_rb_backtrace(rb_err);
            response.internal_server_error(err.to_string());
        } else {
            response.internal_server_error(err.to_string());
        }
    }

    pub fn error(&self, message: String) {
        self.response.internal_server_error(message);
    }

    pub(crate) async fn process_request(
        app: Arc<HeapValue<Proc>>,
        hyper_request: HttpRequest,
        context: &HttpRequestContext,
        script_name: String,
        nonblocking: bool,
    ) -> itsi_error::Result<HttpResponse> {
        match ItsiHttpRequest::new(hyper_request, context, script_name).await {
            Ok((request, receiver)) => {
                let sender = if nonblocking {
                    &context.nonblocking_sender
                } else {
                    &context.job_sender
                };
                match sender.try_send(RequestJob::ProcessHttpRequest(request, app)) {
                    Err(err) => match err {
                        async_channel::TrySendError::Full(_) => Ok(SERVICE_UNAVAILABLE_RESPONSE
                            .to_http_response(context.accept)
                            .await),
                        async_channel::TrySendError::Closed(_) => {
                            error!("Channel closed while sending request job");
                            Ok(INTERNAL_SERVER_ERROR_RESPONSE
                                .to_http_response(context.accept)
                                .await)
                        }
                    },
                    Ok(_) => match receiver.await {
                        Ok(ResponseFrame::HttpResponse(response)) => Ok(response),
                        Ok(ResponseFrame::HijackedResponse(response)) => {
                            match response.process_hijacked_response().await {
                                Ok(result) => Ok(result),
                                Err(e) => {
                                    error!("Error processing hijacked response: {}", e);
                                    Ok(Response::new(HttpBody::empty()))
                                }
                            }
                        }
                        Err(_) => {
                            error!("Failed to receive response from receiver");
                            Ok(INTERNAL_SERVER_ERROR_RESPONSE
                                .to_http_response(context.accept)
                                .await)
                        }
                    },
                }
            }
            Err(err_resp) => Ok(err_resp),
        }
    }

    pub(crate) async fn new(
        request: HttpRequest,
        context: &HttpRequestContext,
        script_name: String,
    ) -> Result<
        (
            ItsiHttpRequest,
            tokio::sync::oneshot::Receiver<ResponseFrame>,
        ),
        HttpResponse,
    > {
        let (parts, body) = request.into_parts();
        let parts = Arc::new(parts);
        let body = if parts.headers.get(CONTENT_LENGTH) == Some(&ZERO_HEADER_VALUE) {
            ItsiBody::Empty
        } else if context.server_params.streamable_body {
            ItsiBody::Stream(ItsiBodyProxy::new(body))
        } else {
            let mut body_bytes = BigBytes::new();
            let mut stream = body.into_data_stream();
            while let Some(chunk) = stream.next().await {
                match chunk {
                    Ok(byte_array) => body_bytes.write_all(&byte_array).unwrap(),
                    Err(e) => {
                        let mut err_resp = Response::new(HttpBody::empty());
                        if e.downcast_ref::<MaxBodySizeReached>().is_some() {
                            *err_resp.status_mut() = StatusCode::PAYLOAD_TOO_LARGE;
                        }
                        return Err(err_resp);
                    }
                }
            }
            ItsiBody::Buffered(body_bytes)
        };
        let (sender, receiver) = tokio::sync::oneshot::channel::<ResponseFrame>();
        Ok((
            Self {
                context: context.clone(),
                version: parts.version,
                response: ItsiHttpResponse::new(
                    parts.clone(),
                    sender,
                    context.service.shutdown_receiver.clone(),
                ),
                start: Instant::now(),
                script_name,
                body,
                parts,
            },
            receiver,
        ))
    }

    pub(crate) fn path(&self) -> MagnusResult<&str> {
        Ok(self
            .parts
            .uri
            .path()
            .strip_prefix(self.script_name()?)
            .unwrap_or(self.parts.uri.path()))
    }

    pub(crate) fn script_name(&self) -> MagnusResult<&str> {
        Ok(self.script_name.trim_end_matches("/"))
    }

    pub(crate) fn query_string(&self) -> MagnusResult<&str> {
        Ok(self.parts.uri.query().unwrap_or(""))
    }

    pub(crate) fn method(&self) -> MagnusResult<&str> {
        Ok(self.parts.method.as_str())
    }

    pub(crate) fn version(&self) -> MagnusResult<&str> {
        Ok(match self.version {
            Version::HTTP_09 => "HTTP/0.9",
            Version::HTTP_10 => "HTTP/1.0",
            Version::HTTP_11 => "HTTP/1.1",
            Version::HTTP_2 => "HTTP/2.0",
            Version::HTTP_3 => "HTTP/3.0",
            _ => "HTTP/Unknown",
        })
    }

    pub(crate) fn rack_protocol(&self) -> MagnusResult<Vec<&str>> {
        Ok(self
            .parts
            .headers
            .get("upgrade")
            .or_else(|| self.parts.headers.get("protocol"))
            .map(|value| {
                value
                    .to_str()
                    .unwrap_or("")
                    .split(',')
                    .map(|s| s.trim())
                    .collect::<Vec<&str>>()
            })
            .unwrap_or_else(|| vec!["http"]))
    }

    pub(crate) fn host(&self) -> MagnusResult<&str> {
        Ok(self
            .parts
            .uri
            .host()
            .unwrap_or_else(|| &self.context.listener_info.host))
    }

    pub(crate) fn scheme(&self) -> MagnusResult<&str> {
        Ok(self
            .parts
            .uri
            .scheme()
            .map(|scheme| scheme.as_str())
            .unwrap_or_else(|| &self.context.listener_info.scheme))
    }

    pub(crate) fn headers(&self) -> MagnusResult<Vec<(&str, &str)>> {
        Ok(self
            .parts
            .headers
            .iter()
            .map(|(hn, hv)| (hn.as_str(), hv.to_str().unwrap_or("")))
            .collect::<Vec<(&str, &str)>>())
    }

    pub(crate) fn each_header(&self) -> MagnusResult<()> {
        self.parts.headers.iter().for_each(|(hn, hv)| {
            yield_values::<_, Value>((hn.as_str(), hv.to_str().unwrap_or(""))).ok();
        });
        Ok(())
    }

    pub(crate) fn uri(&self) -> MagnusResult<String> {
        Ok(self.parts.uri.to_string())
    }

    pub fn header(&self, name: String) -> MagnusResult<Option<Vec<&str>>> {
        let result: Vec<&str> = self
            .parts
            .headers
            .get_all(&name)
            .iter()
            .filter_map(|value| value.to_str().ok())
            .collect();
        Ok(Some(result))
    }

    pub(crate) fn remote_addr(&self) -> MagnusResult<&str> {
        Ok(&self.context.addr)
    }

    pub(crate) fn port(&self) -> MagnusResult<u16> {
        Ok(self
            .parts
            .uri
            .port_u16()
            .unwrap_or(self.context.listener_info.port))
    }

    pub(crate) fn body(&self) -> MagnusResult<Option<Value>> {
        Ok(self.body.into_value())
    }

    pub(crate) fn response(&self) -> MagnusResult<ItsiHttpResponse> {
        Ok(self.response.clone())
    }
}
