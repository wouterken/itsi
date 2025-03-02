use crate::{
    response::itsi_response::ItsiResponse,
    server::{
        itsi_server::RequestJob,
        listener::{Listener, SockAddr},
    },
};
use bytes::Bytes;
use crossbeam::channel::Sender;
use derive_more::Debug;
use http::{request::Parts, Response, StatusCode};
use http_body_util::{combinators::BoxBody, BodyExt, Empty};
use hyper::{body::Incoming, Request};
use itsi_error::Result;
use itsi_tracing::{debug, error};
use magnus::error::Result as MagnusResult;
use magnus::{
    value::{LazyId, Opaque, ReprValue},
    RClass, Ruby, Value,
};
use std::{collections::HashMap, convert::Infallible, sync::Arc};
use tokio::sync::oneshot;

static ID_CALL: LazyId = LazyId::new("call");
static ID_EACH: LazyId = LazyId::new("each");
static ID_CLOSE: LazyId = LazyId::new("close");

#[magnus::wrap(class = "Itsi::Request", free_immediately, size)]
#[derive(Debug)]
pub struct ItsiRequest {
    pub parts: Arc<Parts>,
    pub body: Bytes,
    pub sender: Option<oneshot::Sender<ItsiResponse>>,
    pub remote_addr: String,
    pub version: String,
    #[debug(skip)]
    pub(crate) listener: Arc<Listener>,
    pub script_name: String,
}

impl ItsiRequest {
    pub fn process(mut self, _ruby: &Ruby, server: RClass, app: Opaque<Value>) -> Result<()> {
        let sender = self.sender.take().expect("sender must be present");
        let parts = self.parts.clone();

        match server.funcall::<_, _, (u16, Vec<String>, Value)>(*ID_CALL, (app, self)) {
            Ok((status, headers, body)) => {
                let body_string = body
                    .enumeratorize(*ID_EACH, ())
                    .map(|v| v.unwrap().to_string())
                    .collect::<Vec<String>>()
                    .join("");

                body.check_funcall::<_, _, Value>(*ID_CLOSE, ());

                if let Err(err) = sender.send(ItsiResponse {
                    status,
                    headers,
                    body: body_string,
                    parts,
                }) {
                    debug!("Response Dropped {:?}", err)
                } else {
                    debug!("Request processed. Sending response back to accept thread.");
                }
            }
            Err(err) => {
                error!("Error processing request: {}", err);
            }
        }

        Ok(())
    }

    pub(crate) async fn process_request(
        hyper_request: Request<Incoming>,
        sender: Arc<Sender<RequestJob>>,
        script_name: String,
        listener: Arc<Listener>,
        addr: SockAddr,
    ) -> itsi_error::Result<Response<BoxBody<Bytes, Infallible>>> {
        let (request, receiver) =
            ItsiRequest::build_from(hyper_request, addr, script_name, listener).await;

        debug!("Sending request {:?} to worker thread", request);
        match sender.send(RequestJob::ProcessRequest(request)) {
            Err(err) => {
                error!("Error occurred: {}", err);
                let mut response = Response::new(BoxBody::new(Empty::new()));
                *response.status_mut() = StatusCode::BAD_REQUEST;
                Ok(response)
            }
            _ => match receiver.await {
                Ok(response) => Ok(response.into()),
                Err(err) => {
                    error!("Recv Error occurred: {}", err);
                    let mut response = Response::new(BoxBody::new(Empty::new()));
                    *response.status_mut() = StatusCode::BAD_REQUEST;
                    Ok(response)
                }
            },
        }
    }

    pub(crate) async fn build_from(
        request: Request<Incoming>,
        sock_addr: SockAddr,
        script_name: String,
        listener: Arc<Listener>,
    ) -> (Self, oneshot::Receiver<ItsiResponse>) {
        let (parts, body) = request.into_parts();
        let body = body.collect().await.unwrap().to_bytes();
        let (sender, receiver) = oneshot::channel();
        (
            Self {
                remote_addr: sock_addr.to_string(),
                body,
                script_name,
                listener,
                version: format!("{:?}", &parts.version),
                parts: Arc::new(parts),
                sender: Some(sender),
            },
            receiver,
        )
    }
}

impl ItsiRequest {
    pub(crate) fn path(&self) -> MagnusResult<&str> {
        Ok(self
            .parts
            .uri
            .path()
            .strip_prefix(&self.script_name)
            .unwrap_or(self.parts.uri.path()))
    }

    pub(crate) fn script_name(&self) -> MagnusResult<String> {
        Ok(self.script_name.clone())
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

    pub(crate) fn headers(&self) -> MagnusResult<HashMap<String, &str>> {
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

    pub(crate) fn body(&self) -> MagnusResult<Bytes> {
        Ok(self.body.clone())
    }
}
