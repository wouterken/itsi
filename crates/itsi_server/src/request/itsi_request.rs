use crate::{
    body_proxy::itsi_body_proxy::{BigBytes, ItsiBody, ItsiBodyProxy},
    response::itsi_response::ItsiResponse,
    server::{
        itsi_server::{RequestJob, Server},
        listener::{SockAddr, TokioListener},
        serve_strategy::single_mode::RunningPhase,
    },
};
use bytes::Bytes;
use derive_more::Debug;
use futures::StreamExt;
use http::{request::Parts, Response, StatusCode};
use http_body_util::{combinators::BoxBody, BodyExt, Empty};
use hyper::{body::Incoming, Request};
use itsi_error::from::CLIENT_CONNECTION_CLOSED;
use itsi_tracing::{debug, error};
use magnus::{
    error::{ErrorType, Result as MagnusResult},
    Error,
};
use magnus::{
    value::{LazyId, Opaque, ReprValue},
    RClass, Ruby, Value,
};
use std::{convert::Infallible, fmt, io::Write, sync::Arc, time::Instant};
use tokio::sync::{
    mpsc::{self},
    watch,
};
static ID_CALL: LazyId = LazyId::new("call");
static ID_MESSAGE: LazyId = LazyId::new("message");
static ID_BACKTRACE: LazyId = LazyId::new("backtrace");

#[derive(Debug)]
#[magnus::wrap(class = "Itsi::Request", free_immediately, size)]
pub struct ItsiRequest {
    pub parts: Parts,
    #[debug(skip)]
    pub body: ItsiBody,
    pub remote_addr: String,
    pub version: String,
    #[debug(skip)]
    pub(crate) listener: Arc<TokioListener>,
    #[debug(skip)]
    pub server: Arc<Server>,
    pub response: ItsiResponse,
    pub start: Instant,
}

impl fmt::Display for ItsiRequest {
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

impl ItsiRequest {
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

    pub fn process(
        self,
        ruby: &Ruby,
        server: RClass,
        app: Opaque<Value>,
    ) -> magnus::error::Result<()> {
        let req = format!("{}", self);
        let response = self.response.clone();
        let start = self.start;
        debug!("{} Started", req);
        let result = server.funcall::<_, _, Value>(*ID_CALL, (app, self));
        if let Err(err) = result {
            Self::internal_error(ruby, response, err);
        }
        debug!("{} Finished in {:?}", req, start.elapsed());

        Ok(())
    }

    pub fn internal_error(ruby: &Ruby, response: ItsiResponse, err: Error) {
        if Self::is_connection_closed_err(ruby, &err) {
            debug!("Connection closed by client");
            response.close();
        } else if let Some(rb_err) = err.value() {
            let backtrace = rb_err
                .funcall::<_, _, Vec<String>>(*ID_BACKTRACE, ())
                .unwrap_or_default();

            error!("Error occurred in Handler: {:?}", rb_err);
            for line in backtrace {
                error!("{}", line);
            }
            response.internal_server_error(err.to_string());
        } else {
            response.internal_server_error(err.to_string());
        }
    }

    pub fn error(self, message: String) {
        self.response.internal_server_error(message);
    }

    pub(crate) async fn process_request(
        hyper_request: Request<Incoming>,
        sender: async_channel::Sender<RequestJob>,
        server: Arc<Server>,
        listener: Arc<TokioListener>,
        addr: SockAddr,
        shutdown_rx: watch::Receiver<RunningPhase>,
    ) -> itsi_error::Result<Response<BoxBody<Bytes, Infallible>>> {
        let (request, mut receiver) = ItsiRequest::new(hyper_request, addr, server, listener).await;

        let response = request.response.clone();
        match sender.send(RequestJob::ProcessRequest(request)).await {
            Err(err) => {
                error!("Error occurred: {}", err);
                let mut response = Response::new(BoxBody::new(Empty::new()));
                *response.status_mut() = StatusCode::BAD_REQUEST;
                Ok(response)
            }
            _ => match receiver.recv().await {
                Some(first_frame) => Ok(response.build(first_frame, receiver, shutdown_rx).await),
                None => Ok(response.build(None, receiver, shutdown_rx).await),
            },
        }
    }

    pub(crate) async fn new(
        request: Request<Incoming>,
        sock_addr: SockAddr,
        server: Arc<Server>,
        listener: Arc<TokioListener>,
    ) -> (ItsiRequest, mpsc::Receiver<Option<Bytes>>) {
        let (parts, body) = request.into_parts();
        let body = if server.stream_body.is_some_and(|f| f) {
            ItsiBody::Stream(ItsiBodyProxy::new(body))
        } else {
            let mut body_bytes = BigBytes::new();
            let mut stream = body.into_data_stream();
            while let Some(chunk) = stream.next().await {
                let byte_array = chunk.unwrap().to_vec();
                body_bytes.write_all(&byte_array).unwrap();
            }
            ItsiBody::Buffered(body_bytes)
        };
        let response_channel = mpsc::channel::<Option<Bytes>>(100);
        (
            Self {
                remote_addr: sock_addr.to_string(),
                body,
                server,
                listener,
                version: format!("{:?}", &parts.version),
                response: ItsiResponse::new(parts.clone(), response_channel.0),
                start: Instant::now(),
                parts,
            },
            response_channel.1,
        )
    }

    pub(crate) fn path(&self) -> MagnusResult<&str> {
        Ok(self
            .parts
            .uri
            .path()
            .strip_prefix(&self.server.script_name)
            .unwrap_or(self.parts.uri.path()))
    }

    pub(crate) fn script_name(&self) -> MagnusResult<&str> {
        Ok(&self.server.script_name)
    }

    pub(crate) fn query_string(&self) -> MagnusResult<&str> {
        Ok(self.parts.uri.query().unwrap_or(""))
    }

    pub(crate) fn method(&self) -> MagnusResult<&str> {
        Ok(self.parts.method.as_str())
    }

    pub(crate) fn version(&self) -> MagnusResult<&str> {
        Ok(&self.version)
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

    pub(crate) fn host(&self) -> MagnusResult<String> {
        Ok(self
            .parts
            .uri
            .host()
            .map(|host| host.to_string())
            .unwrap_or_else(|| self.listener.host()))
    }

    pub(crate) fn scheme(&self) -> MagnusResult<String> {
        Ok(self
            .parts
            .uri
            .scheme()
            .map(|scheme| scheme.to_string())
            .unwrap_or_else(|| self.listener.scheme()))
    }

    pub(crate) fn headers(&self) -> MagnusResult<Vec<(String, &str)>> {
        Ok(self
            .parts
            .headers
            .iter()
            .map(|(hn, hv)| {
                let key = match hn.as_str() {
                    "content-length" => "CONTENT_LENGTH".to_string(),
                    "content-type" => "CONTENT_TYPE".to_string(),
                    _ => format!("HTTP_{}", hn.as_str().to_uppercase().replace("-", "_")),
                };
                (key, hv.to_str().unwrap_or(""))
            })
            .collect())
    }

    pub(crate) fn remote_addr(&self) -> MagnusResult<&str> {
        Ok(&self.remote_addr)
    }

    pub(crate) fn port(&self) -> MagnusResult<u16> {
        Ok(self.parts.uri.port_u16().unwrap_or(self.listener.port()))
    }

    pub(crate) fn body(&self) -> MagnusResult<Value> {
        Ok(self.body.into_value())
    }

    pub(crate) fn response(&self) -> MagnusResult<ItsiResponse> {
        Ok(self.response.clone())
    }
}
