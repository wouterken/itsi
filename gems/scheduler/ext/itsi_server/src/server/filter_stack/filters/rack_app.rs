use super::FilterLayer;
use crate::{
    ruby_types::itsi_http_request::ItsiHttpRequest,
    server::{
        itsi_service::ItsiService,
        types::{HttpRequest, HttpResponse},
    },
};
use async_trait::async_trait;
use derive_more::Debug;
use either::Either;
use itsi_rb_helpers::HeapVal;
use magnus::{block::Proc, error::Result, value::ReprValue, Symbol, Value};
use std::sync::Arc;
use tracing::info;

#[derive(Debug)]
pub struct RackApp {
    app: Arc<HeapVal>,
}

impl RackApp {
    pub fn from_value(params: HeapVal) -> magnus::error::Result<Self> {
        let loader = params.funcall::<_, _, Proc>(Symbol::new("[]"), ("rackup_loader",))?;
        let app = loader.call::<_, Value>(())?;
        Ok(RackApp {
            app: Arc::new(app.into()),
        })
    }
}

#[async_trait]
impl FilterLayer for RackApp {
    async fn before(
        &self,
        req: HttpRequest,
        context: &ItsiService,
    ) -> Result<Either<HttpRequest, HttpResponse>> {
        ItsiHttpRequest::process_request(self.app.clone(), req, context)
            .await
            .map_err(|e| e.into())
            .map(Either::Right)
    }

    /// The “after” hook. By default, it passes through the response.
    async fn after(&self, resp: HttpResponse) -> HttpResponse {
        resp
    }
}
