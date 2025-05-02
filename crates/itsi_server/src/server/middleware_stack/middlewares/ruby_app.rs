use super::MiddlewareLayer;
use crate::ruby_types::itsi_grpc_call::ItsiGrpcCall;
use crate::ruby_types::itsi_http_request::ItsiHttpRequest;
use crate::server::http_message_types::{HttpRequest, HttpResponse};
use crate::services::itsi_http_service::HttpRequestContext;
use crate::services::static_file_server::ROOT_STATIC_FILE_SERVER;
use async_trait::async_trait;
use derive_more::Debug;
use either::Either;
use itsi_rb_helpers::{HeapVal, HeapValue};
use magnus::{block::Proc, error::Result, value::ReprValue, Symbol};
use regex::Regex;
use std::str::FromStr;
use std::sync::atomic::Ordering;
use std::sync::Arc;

#[derive(Debug)]
pub struct RubyApp {
    app: Arc<HeapValue<Proc>>,
    request_type: RequestType,
    script_name: Option<String>,
    sendfile: bool,
    nonblocking: bool,
    base_path: Regex,
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
    pub fn from_value(params: HeapVal) -> magnus::error::Result<Arc<Self>> {
        let app = params.funcall::<_, _, Proc>(Symbol::new("[]"), ("app_proc",))?;
        let sendfile = params
            .funcall::<_, _, bool>(Symbol::new("[]"), ("sendfile",))
            .unwrap_or(true);
        let nonblocking = params
            .funcall::<_, _, bool>(Symbol::new("[]"), ("nonblocking",))
            .unwrap_or(false);
        let base_path_src = params
            .funcall::<_, _, String>(Symbol::new("[]"), ("base_path",))
            .unwrap_or("".to_owned());
        let script_name = params
            .funcall::<_, _, Option<String>>(Symbol::new("[]"), ("script_name",))
            .unwrap_or(None);
        let base_path = Regex::new(&base_path_src).unwrap();

        let request_type: RequestType = params
            .funcall::<_, _, String>(Symbol::new("[]"), ("request_type",))
            .unwrap_or("http".to_string())
            .parse()
            .unwrap_or(RequestType::Http);

        Ok(Arc::new(RubyApp {
            app: Arc::new(app.into()),
            sendfile,
            nonblocking,
            script_name,
            request_type,
            base_path,
        }))
    }
}

#[async_trait]
impl MiddlewareLayer for RubyApp {
    async fn before(
        &self,
        req: HttpRequest,
        context: &mut HttpRequestContext,
    ) -> Result<Either<HttpRequest, HttpResponse>> {
        context.is_ruby_request.store(true, Ordering::SeqCst);
        match self.request_type {
            RequestType::Http => {
                let uri = req.uri().path();
                let script_name = self.script_name.clone().unwrap_or_else(|| {
                    self.base_path
                        .captures(uri)
                        .and_then(|caps| caps.name("base_path"))
                        .map(|m| m.as_str())
                        .unwrap_or("/")
                        .to_owned()
                });
                ItsiHttpRequest::process_request(
                    self.app.clone(),
                    req,
                    context,
                    script_name,
                    self.nonblocking,
                )
                .await
                .map_err(|e| e.into())
                .map(Either::Right)
            }
            RequestType::Grpc => {
                ItsiGrpcCall::process_request(self.app.clone(), req, context, self.nonblocking)
                    .await
                    .map_err(|e| e.into())
                    .map(Either::Right)
            }
        }
    }

    async fn after(&self, resp: HttpResponse, context: &mut HttpRequestContext) -> HttpResponse {
        if self.sendfile {
            if let Some(sendfile_header) = resp.headers().get("X-Sendfile") {
                return ROOT_STATIC_FILE_SERVER
                    .serve_single_abs(sendfile_header.to_str().unwrap(), context.accept, &[])
                    .await;
            }
        }
        resp
    }
}
