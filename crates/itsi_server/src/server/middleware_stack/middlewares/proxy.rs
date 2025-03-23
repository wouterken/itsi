use std::{
    collections::HashMap,
    convert::Infallible,
    net::SocketAddr,
    sync::{Arc, OnceLock},
    time::Duration,
};

use crate::server::{
    bind::{Bind, BindAddress},
    itsi_service::RequestContext,
    types::{HttpRequest, HttpResponse},
};

use super::{string_rewrite::StringRewrite, FromValue, MiddlewareLayer};

use async_trait::async_trait;
use either::Either;
use futures::TryStreamExt;
use http::Response;
use http_body_util::{combinators::BoxBody, BodyExt, StreamBody};
use hyper::body::Frame;
use magnus::error::Result;
use reqwest::{dns::Resolve, Body, Client, Url};
use serde::Deserialize;
use tracing::*;

#[derive(Debug, Clone, Deserialize)]
pub struct Proxy {
    pub to: StringRewrite,
    pub backends: Vec<String>,
    pub headers: HashMap<String, Option<ProxiedHeader>>,
    pub verify_ssl: bool,
    pub timeout: u64,
    pub tls_sni: bool,
    #[serde(skip_deserializing)]
    pub client: OnceLock<Client>,
}

#[derive(Debug, Clone, Deserialize)]
pub enum ProxiedHeader {
    #[serde(rename(deserialize = "value"))]
    String(String),
    #[serde(rename(deserialize = "rewrite"))]
    StringRewrite(StringRewrite),
}

impl ProxiedHeader {
    pub fn to_string(&self, req: &HttpRequest, context: &RequestContext) -> String {
        match self {
            ProxiedHeader::String(value) => value.clone(),
            ProxiedHeader::StringRewrite(rewrite) => rewrite.rewrite(req, context),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Resolver {
    backends: Arc<Vec<SocketAddr>>,
}

/// An iterator that owns an Arc to the backend list and iterates over it.
pub struct ResolverIter {
    backends: Arc<Vec<SocketAddr>>,
    index: usize,
}

impl Iterator for ResolverIter {
    type Item = SocketAddr;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.backends.len() {
            let addr = self.backends[self.index];
            self.index += 1;
            Some(addr)
        } else {
            None
        }
    }
}

impl Resolve for Resolver {
    fn resolve(&self, _name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        let backends = self.backends.clone();
        let fut = async move {
            let iter = ResolverIter { backends, index: 0 };
            Ok(Box::new(iter) as Box<dyn Iterator<Item = SocketAddr> + Send>)
        };
        Box::pin(fut)
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
                    magnus::exception::exception(),
                    "Failed to save resolver backends",
                )
            })?;
        Ok(())
    }

    async fn before(
        &self,
        req: HttpRequest,
        context: &mut RequestContext,
    ) -> Result<Either<HttpRequest, HttpResponse>> {
        let url = self.to.rewrite(&req, context);
        let destination = Url::parse(&url).map_err(|e| {
            magnus::Error::new(
                magnus::exception::exception(),
                format!("Failed to create route set: {}", e),
            )
        })?;

        let host_str = destination.host_str().unwrap_or_else(|| {
            req.headers()
                .get("Host")
                .and_then(|h| h.to_str().ok())
                .unwrap_or("")
        });

        let mut reqwest_builder = self
            .client
            .get()
            .unwrap()
            .request(req.method().clone(), url);

        // Forward incoming headers unless they're in remove_headers or overridden.
        for (name, value) in req.headers().iter() {
            let name_str = name.as_str();
            if self.headers.contains_key(name_str) {
                continue;
            }
            reqwest_builder = reqwest_builder.header(name, value);
        }

        // Add the host header if it's not overridden and host_str is non-empty.
        if !self.headers.contains_key("host") && !host_str.is_empty() {
            reqwest_builder = reqwest_builder.header("Host", host_str);
        }

        // Add overriding headers.
        for (name, header_value) in self.headers.iter() {
            if let Some(header_value) = header_value {
                reqwest_builder =
                    reqwest_builder.header(name, header_value.to_string(&req, context));
            }
        }

        let reqwest_builder = reqwest_builder.body(Body::wrap_stream(req.into_data_stream()));
        let reqwest_response = reqwest_builder.send().await.map_err(|e| {
            error!("Failed to build Reqwest response: {:?}", e);
            magnus::Error::new(
                magnus::exception::runtime_error(),
                format!("Reqwest request failed: {}", e),
            )
        })?;

        let status = reqwest_response.status();
        let mut builder = Response::builder().status(status);
        for (hn, hv) in reqwest_response.headers() {
            builder = builder.header(hn, hv);
        }
        let response = builder
            .body(BoxBody::new(StreamBody::new(
                reqwest_response
                    .bytes_stream()
                    .map_ok(Frame::data)
                    .map_err(|_| -> Infallible { unreachable!("We handle IO errors above") }),
            )))
            .map_err(|e| {
                error!("Failed to build Hyper response: {}", e);
                magnus::Error::new(
                    magnus::exception::runtime_error(),
                    format!("Failed to build Hyper response: {}", e),
                )
            })?;

        Ok(Either::Right(response))
    }
}
impl FromValue for Proxy {}
