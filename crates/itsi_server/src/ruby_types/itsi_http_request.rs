use derive_more::Debug;
use futures::StreamExt;
use http::{request::Parts, Response, StatusCode, Version};
use http_body_util::{combinators::BoxBody, BodyExt, Empty};
use itsi_error::CLIENT_CONNECTION_CLOSED;
use itsi_rb_helpers::{print_rb_backtrace, HeapValue};
use itsi_tracing::{debug, error};
use magnus::{
    block::Proc,
    error::{ErrorType, Result as MagnusResult},
    Error,
};
use magnus::{
    value::{LazyId, ReprValue},
    Ruby, Value,
};
use std::{fmt, io::Write, sync::Arc, time::Instant};
use tokio::sync::mpsc::{self};

use super::{
    itsi_body_proxy::{big_bytes::BigBytes, ItsiBody, ItsiBodyProxy},
    itsi_http_response::ItsiHttpResponse,
};
use crate::{
    server::{
        byte_frame::ByteFrame,
        http_message_types::{HttpRequest, HttpResponse},
        request_job::RequestJob,
        size_limited_incoming::MaxBodySizeReached,
    },
    services::itsi_http_service::HttpRequestContext,
};

static ID_MESSAGE: LazyId = LazyId::new("message");

#[derive(Debug)]
#[magnus::wrap(class = "Itsi::HttpRequest", free_immediately, size)]
pub struct ItsiHttpRequest {
    pub parts: Parts,
    #[debug(skip)]
    pub body: ItsiBody,
    pub version: Version,
    pub response: ItsiHttpResponse,
    pub start: Instant,
    #[debug(skip)]
    pub context: HttpRequestContext,
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
    fn content_type_str(&self) -> &str {
        self.parts
            .headers
            .get("Content-Type")
            .and_then(|hv| hv.to_str().ok())
            .unwrap_or("application/x-www-form-urlencoded")
    }

    pub fn is_json(&self) -> bool {
        self.content_type_str() == "application/json"
    }

    pub fn is_html(&self) -> bool {
        self.content_type_str() == "text/html"
    }

    pub fn process(self, ruby: &Ruby, app_proc: Arc<HeapValue<Proc>>) -> magnus::error::Result<()> {
        let response = self.response.clone();
        let result = app_proc.call::<_, Value>((self,));
        if let Err(err) = result {
            Self::internal_error(ruby, response, err);
        }
        Ok(())
    }

    pub fn internal_error(ruby: &Ruby, response: ItsiHttpResponse, err: Error) {
        if Self::is_connection_closed_err(ruby, &err) {
            debug!("Connection closed by client");
            response.close();
        } else if let Some(rb_err) = err.value() {
            print_rb_backtrace(rb_err);
            response.internal_server_error(err.to_string());
        } else {
            response.internal_server_error(err.to_string());
        }
    }

    pub fn error(self, message: String) {
        self.response.internal_server_error(message);
    }

    pub(crate) async fn process_request(
        app: Arc<HeapValue<Proc>>,
        hyper_request: HttpRequest,
        context: &HttpRequestContext,
    ) -> itsi_error::Result<HttpResponse> {
        match ItsiHttpRequest::new(hyper_request, context).await {
            Ok((request, mut receiver)) => {
                let shutdown_channel = context.service.shutdown_channel.clone();
                let response = request.response.clone();
                match context
                    .sender
                    .send(RequestJob::ProcessHttpRequest(request, app))
                    .await
                {
                    Err(err) => {
                        error!("Error occurred: {}", err);
                        let mut response = Response::new(BoxBody::new(Empty::new()));
                        *response.status_mut() = StatusCode::BAD_REQUEST;
                        Ok(response)
                    }
                    _ => match receiver.recv().await {
                        Some(first_frame) => Ok(response
                            .build(first_frame, receiver, shutdown_channel)
                            .await),
                        None => Ok(response
                            .build(ByteFrame::Empty, receiver, shutdown_channel)
                            .await),
                    },
                }
            }
            Err(err_resp) => Ok(err_resp),
        }
    }

    pub(crate) async fn new(
        request: HttpRequest,
        context: &HttpRequestContext,
    ) -> Result<(ItsiHttpRequest, mpsc::Receiver<ByteFrame>), HttpResponse> {
        let (parts, body) = request.into_parts();
        let body = if context.server_params.streamable_body {
            ItsiBody::Stream(ItsiBodyProxy::new(body))
        } else {
            let mut body_bytes = BigBytes::new();
            let mut stream = body.into_data_stream();
            while let Some(chunk) = stream.next().await {
                match chunk {
                    Ok(byte_array) => body_bytes.write_all(&byte_array).unwrap(),
                    Err(e) => {
                        let mut err_resp = Response::new(BoxBody::new(Empty::new()));
                        if e.downcast_ref::<MaxBodySizeReached>().is_some() {
                            *err_resp.status_mut() = StatusCode::PAYLOAD_TOO_LARGE;
                        }
                        return Err(err_resp);
                    }
                }
            }
            ItsiBody::Buffered(body_bytes)
        };
        let response_channel = mpsc::channel::<ByteFrame>(100);
        Ok((
            Self {
                context: context.clone(),
                version: parts.version,
                response: ItsiHttpResponse::new(parts.clone(), response_channel.0),
                start: Instant::now(),
                body,
                parts,
            },
            response_channel.1,
        ))
    }

    pub(crate) fn path(&self) -> MagnusResult<&str> {
        Ok(self
            .parts
            .uri
            .path()
            .strip_prefix(&self.context.server_params.script_name)
            .unwrap_or(self.parts.uri.path()))
    }

    pub(crate) fn script_name(&self) -> MagnusResult<&str> {
        Ok(&self.context.server_params.script_name)
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
            .unwrap_or_else(|| &self.context.listener.host))
    }

    pub(crate) fn scheme(&self) -> MagnusResult<&str> {
        Ok(self
            .parts
            .uri
            .scheme()
            .map(|scheme| scheme.as_str())
            .unwrap_or_else(|| &self.context.listener.scheme))
    }

    pub(crate) fn headers(&self) -> MagnusResult<Vec<(&str, &str)>> {
        Ok(self
            .parts
            .headers
            .iter()
            .map(|(hn, hv)| (hn.as_str(), hv.to_str().unwrap_or("")))
            .collect::<Vec<(&str, &str)>>())
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
            .unwrap_or(self.context.listener.port))
    }

    pub(crate) fn body(&self) -> MagnusResult<Option<Value>> {
        Ok(self.body.into_value())
    }

    pub(crate) fn response(&self) -> MagnusResult<ItsiHttpResponse> {
        Ok(self.response.clone())
    }
}
