use super::MiddlewareLayer;
use crate::ruby_types::itsi_grpc_request::ItsiGrpcRequest;
use crate::server::static_file_server::ROOT_STATIC_FILE_SERVER;
use crate::{
    ruby_types::itsi_http_request::ItsiHttpRequest,
    server::{
        itsi_service::RequestContext,
        types::{HttpRequest, HttpResponse},
    },
};
use async_trait::async_trait;
use derive_more::Debug;
use either::Either;
use itsi_rb_helpers::{HeapVal, HeapValue};
use magnus::{block::Proc, error::Result, value::ReprValue, Symbol};
use std::str::FromStr;
use std::sync::Arc;
use tracing::info;

#[derive(Debug)]
pub struct RubyApp {
    app: Arc<HeapValue<Proc>>,
    request_type: RequestType,
    sendfile: bool,
}

#[derive(Debug)]
pub enum RequestType {
    Http,
    Grpc,
}

impl FromStr for RequestType {
    type Err = &'static str;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "http" => Ok(RequestType::Http),
            "grpc" => Ok(RequestType::Grpc),
            _ => Err("Invalid request type"),
        }
    }
}

impl RubyApp {
    pub fn from_value(params: HeapVal) -> magnus::error::Result<Self> {
        let app = params.funcall::<_, _, Proc>(Symbol::new("[]"), ("app_proc",))?;
        let sendfile = params
            .funcall::<_, _, bool>(Symbol::new("[]"), ("sendfile",))
            .unwrap_or(true);
        let request_type: RequestType = params
            .funcall::<_, _, String>(Symbol::new("[]"), ("request_type",))
            .unwrap_or("http".to_string())
            .parse()
            .unwrap_or(RequestType::Http);
        info!("Request type {:?}", request_type);
        Ok(RubyApp {
            app: Arc::new(app.into()),
            sendfile,
            request_type,
        })
    }
}

#[async_trait]
impl MiddlewareLayer for RubyApp {
    async fn before(
        &self,
        req: HttpRequest,
        context: &mut RequestContext,
    ) -> Result<Either<HttpRequest, HttpResponse>> {
        match self.request_type {
            RequestType::Http => ItsiHttpRequest::process_request(self.app.clone(), req, context)
                .await
                .map_err(|e| e.into())
                .map(Either::Right),
            RequestType::Grpc => ItsiGrpcRequest::process_request(self.app.clone(), req, context)
                .await
                .map_err(|e| e.into())
                .map(Either::Right),
        }
    }

    async fn after(&self, resp: HttpResponse, _context: &mut RequestContext) -> HttpResponse {
        if self.sendfile {
            if let Some(sendfile_header) = resp.headers().get("X-Sendfile") {
                return ROOT_STATIC_FILE_SERVER
                    .serve_single(sendfile_header.to_str().unwrap())
                    .await;
            }
        }
        resp
    }
}
