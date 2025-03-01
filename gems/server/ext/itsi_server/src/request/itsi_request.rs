use std::{collections::HashMap, sync::Arc};

use crate::{
    response::itsi_response::ItsiResponse,
    server::{
        itsi_server::Server,
        listener::{Listener, SockAddr},
    },
};
use bytes::Bytes;
use http::request::Parts;
use http_body_util::BodyExt;
use hyper::{body::Incoming, Request};
use itsi_error::Result;
use itsi_tracing::{debug, error, info};
use magnus::error::Result as MagnusResult;
use magnus::{
    value::{LazyId, Opaque, OpaqueId, ReprValue},
    IntoValue, RClass, Ruby, Value,
};
use tokio::sync::oneshot;

static ID_CALL: LazyId = LazyId::new("call");

#[magnus::wrap(class = "Itsi::Request", free_immediately, size)]
#[derive(Debug)]
pub struct ItsiRequest {
    pub path: String,
    pub script_name: String,
    pub query_string: String,
    pub method: String,
    pub version: String,
    pub rack_protocol: Vec<String>,
    pub host: String,
    pub scheme: String,
    pub headers: HashMap<String, String>,
    pub remote_addr: String,
    pub port: u16,
    pub body: Bytes,
    pub parts: Arc<Parts>,
    pub sender: Option<oneshot::Sender<ItsiResponse>>,
}

impl ItsiRequest {
    pub fn process(mut self, _ruby: &Ruby, server: RClass, app: Opaque<Value>) -> Result<()> {
        let sender = self.sender.take().expect("sender must be present");
        let parts = self.parts.clone();

        match server.funcall::<_, _, (u16, Vec<(String, String)>, Value)>(*ID_CALL, (app, self)) {
            Ok((status, headers, body)) => {
                let body_string = body
                    .enumeratorize("each", ())
                    .map(|v| v.unwrap().to_string())
                    .collect::<Vec<String>>()
                    .join("");

                body.check_funcall::<_, _, Value>("close", ());

                let response = ItsiResponse {
                    status,
                    headers,
                    body: body_string,
                    parts,
                };
                debug!("Request processed. Sending response back to accept thread.");
                if let Err(err) = sender.send(response) {
                    info!("Response Dropped {:?}", err)
                }
            }
            Err(err) => {
                error!("Error processing request: {}", err);
            }
        }

        Ok(())
    }

    pub(crate) async fn build_from(
        request: Request<Incoming>,
        sock_addr: SockAddr,
        script_name: String,
        listener: Arc<Listener>,
    ) -> (Self, oneshot::Receiver<ItsiResponse>) {
        let (parts, body) = request.into_parts();
        let method = parts.method.to_string();
        let port = parts.uri.port_u16().unwrap_or(listener.port());
        let query_string = parts.uri.query().unwrap_or("").to_string();
        let rack_protocol = parts
            .headers
            .get("upgrade")
            .or_else(|| parts.headers.get("protocol"))
            .map(|value| {
                value
                    .to_str()
                    .unwrap_or("")
                    .split(',')
                    .map(|s| s.trim().to_owned())
                    .collect::<Vec<String>>()
            })
            .unwrap_or_else(|| vec!["http".to_string()]);

        let host = parts
            .uri
            .host()
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| listener.host());

        let scheme = parts
            .uri
            .scheme()
            .map(|s| s.to_string())
            .unwrap_or_else(|| listener.scheme());

        let headers = parts
            .headers
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();

        let path = parts
            .uri
            .path()
            .strip_prefix(&script_name)
            .unwrap_or(parts.uri.path())
            .to_string();

        let version = format!("{:?}", parts.version);
        let body = body.collect().await.unwrap().to_bytes();

        let (sender, receiver) = oneshot::channel();
        (
            Self {
                remote_addr: sock_addr.to_string(),
                body,
                script_name,
                query_string,
                method,
                headers,
                path,
                version,
                rack_protocol,
                host,
                scheme,
                port,
                parts: Arc::new(parts),
                sender: Some(sender),
            },
            receiver,
        )
    }
}

impl ItsiRequest {
    pub(crate) fn path(&self) -> MagnusResult<String> {
        Ok(self.path.clone())
    }

    pub(crate) fn script_name(&self) -> MagnusResult<String> {
        Ok(self.script_name.clone())
    }

    pub(crate) fn query_string(&self) -> MagnusResult<String> {
        Ok(self.query_string.clone())
    }

    pub(crate) fn method(&self) -> MagnusResult<String> {
        Ok(self.method.clone())
    }

    pub(crate) fn version(&self) -> MagnusResult<String> {
        Ok(self.version.clone())
    }

    pub(crate) fn rack_protocol(&self) -> MagnusResult<Vec<String>> {
        Ok(self.rack_protocol.clone())
    }

    pub(crate) fn host(&self) -> MagnusResult<String> {
        Ok(self.host.clone())
    }

    pub(crate) fn headers(&self) -> MagnusResult<HashMap<String, String>> {
        Ok(self.headers.clone())
    }

    pub(crate) fn remote_addr(&self) -> MagnusResult<String> {
        Ok(self.remote_addr.clone())
    }

    pub(crate) fn port(&self) -> MagnusResult<u16> {
        Ok(self.port)
    }

    pub(crate) fn body(&self) -> MagnusResult<Bytes> {
        Ok(self.body.clone())
    }
}
