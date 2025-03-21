use super::MiddlewareLayer;
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
use itsi_rb_helpers::{HeapVal, HeapValue};
use magnus::{block::Proc, error::Result, value::ReprValue, Symbol};
use std::sync::Arc;

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
        context: &ItsiService,
    ) -> Result<Either<HttpRequest, HttpResponse>> {
        ItsiHttpRequest::process_request(self.app.clone(), req, context)
            .await
            .map_err(|e| e.into())
            .map(Either::Right)
    }
}
