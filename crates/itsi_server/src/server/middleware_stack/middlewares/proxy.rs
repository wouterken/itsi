use std::{
    collections::HashMap, convert::Infallible, error::Error, net::SocketAddr, sync::Arc,
    time::Duration,
};

use crate::server::{
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
use rand::seq::IndexedRandom;
use reqwest::{dns::Resolve, Body, Client};
use rustls::{ClientConfig, RootCertStore};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Proxy {
    pub to: Vec<StringRewrite>,
    pub headers: HashMap<String, ProxiedHeader>,
    pub verify_ssl: bool,
    pub timeout: u64,
    pub tls_sni: bool,
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

pub struct Resolver {
    records: HashMap<String, Vec<SocketAddr>>,
}

impl Resolver {
    /// Create a new Resolver with a pre-defined mapping.
    pub fn new(records: HashMap<String, Vec<SocketAddr>>) -> Self {
        Resolver { records }
    }
}

impl Resolve for Resolver {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        // Convert the provided Name to a String for lookup.
        let hostname = name.as_str().to_owned();
        // Clone the stored addresses for this hostname, if any.
        let addresses = self.records.get(&hostname).cloned();
        // Create an async block to return the addresses as an iterator.
        let fut = async move {
            if let Some(addrs) = addresses {
                // Return the addresses as an iterator.
                Ok(Box::new(addrs.into_iter()) as Box<dyn Iterator<Item = SocketAddr> + Send>)
            } else {
                // Return an error if the hostname isn't found.
                Err(Box::<dyn Error + Send + Sync>::from(format!(
                    "Hostname {} not found in custom resolver",
                    hostname
                )))
            }
        };
        Box::pin(fut)
    }
}

#[async_trait]
impl MiddlewareLayer for Proxy {
    async fn before(
        &self,
        req: HttpRequest,
        context: &mut RequestContext,
    ) -> Result<Either<HttpRequest, HttpResponse>> {
        let destination = {
            let mut rng = rand::rngs::ThreadRng::default();
            self.to
                .choose(&mut rng)
                .expect("destination list cannot be empty")
                .rewrite(&req, context)
        };

        // Build a Reqwest client with the given timeout and SSL settings.
        let client = Client::builder()
            .timeout(Duration::from_secs(self.timeout))
            .danger_accept_invalid_certs(!self.verify_ssl)
            .danger_accept_invalid_hostnames(!self.verify_ssl)
            .dns_resolver(Arc::new(Resolver::new(HashMap::new())))
            .tls_sni(self.tls_sni)
            .build()
            .map_err(|e| {
                magnus::Error::new(
                    magnus::exception::runtime_error(),
                    format!("Failed to build Reqwest client: {}", e),
                )
            })?;

        let mut reqwest_builder = client.request(req.method().clone(), &destination);

        // Forward headers from the incoming request.
        for (name, value) in req.headers().iter() {
            if !self.headers.contains_key(name.as_str()) {
                reqwest_builder = reqwest_builder.header(name, value);
            }
        }
        for (name, value) in self.headers.iter() {
            reqwest_builder = reqwest_builder.header(name, value.to_string(&req, context));
        }
        let reqwest_builder = reqwest_builder.body(Body::wrap_stream(req.into_data_stream()));
        let reqwest_response = reqwest_builder.send().await.map_err(|e| {
            magnus::Error::new(
                magnus::exception::runtime_error(),
                format!("Reqwest request failed: {}", e),
            )
        })?;

        let status = reqwest_response.status();
        let mut headers = reqwest_response.headers().clone();

        let mut builder = Response::builder().status(status);
        builder.headers_mut().replace(&mut headers);
        let response = builder
            .body(BoxBody::new(StreamBody::new(
                reqwest_response
                    .bytes_stream()
                    .map_ok(Frame::data)
                    .map_err(|_| -> Infallible { unreachable!("We handle IO errors above") }),
            )))
            .map_err(|e| {
                magnus::Error::new(
                    magnus::exception::runtime_error(),
                    format!("Failed to build Hyper response: {}", e),
                )
            })?;

        Ok(Either::Right(response))
    }
}
impl FromValue for Proxy {}
