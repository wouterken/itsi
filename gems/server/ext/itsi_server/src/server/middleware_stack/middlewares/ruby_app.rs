use super::MiddlewareLayer;
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
use std::sync::Arc;
use tracing::info;

#[derive(Debug)]
pub struct RubyApp {
    app: Arc<HeapValue<Proc>>,
}

impl RubyApp {
    pub fn from_value(params: HeapVal) -> magnus::error::Result<Self> {
        let app = params.funcall::<_, _, Proc>(Symbol::new("[]"), ("app_proc",))?;
        Ok(RubyApp {
            app: Arc::new(app.into()),
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
        ItsiHttpRequest::process_request(self.app.clone(), req, context)
            .await
            .map_err(|e| e.into())
            .map(Either::Right)
    }

    async fn after(&self, resp: HttpResponse, _context: &mut RequestContext) -> HttpResponse {
        info!("Checking for X-Sendfile header in {:?}", resp.headers());
        if let Some(sendfile_header) = resp.headers().get("X-Sendfile") {
            ROOT_STATIC_FILE_SERVER
                .serve_single(sendfile_header.to_str().unwrap())
                .await
        } else {
            resp
        }
    }
}
