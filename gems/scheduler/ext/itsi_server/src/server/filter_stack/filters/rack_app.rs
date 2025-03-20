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
use magnus::{
    error::{self, Result},
    value::{LazyId, ReprValue},
    Ruby, Symbol, Value,
};
use std::sync::{Arc, OnceLock};
use tracing::info;

#[derive(Debug)]
pub struct RackApp {
    app: OnceLock<Arc<HeapVal>>,
    app_loader: HeapVal,
}
impl RackApp {
    pub(crate) fn preload(&self) -> error::Result<()> {
        let app = HeapVal::from(self.app_loader.funcall::<_, _, Value>(*ID_CALL, ())?);
        self.app.set(Arc::new(app)).map_err(|e| {
            magnus::Error::new(
                magnus::exception::exception(),
                format!("Failed to preload app {:?}", e),
            )
        })?;
        Ok(())
    }
}

static ID_ACCESSOR: LazyId = LazyId::new("[]");
static ID_CALL: LazyId = LazyId::new("call");

impl RackApp {
    pub fn from_value(value: HeapVal) -> magnus::error::Result<Self> {
        let ruby = Ruby::get().unwrap();
        let app: HeapVal = if value.is_kind_of(ruby.class_hash()) {
            value
                .funcall::<_, _, Value>(*ID_ACCESSOR, (Symbol::new("app"),))?
                .into()
        } else {
            value
        };
        info!("Creating RackApp filter with app: {:?}", app);
        Ok(RackApp {
            app_loader: app.into(),
            app: OnceLock::new(),
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
        let app = self.app.get().unwrap().clone();
        ItsiHttpRequest::process_request(app, req, context)
            .await
            .map_err(|e| e.into())
            .map(Either::Right)
    }

    /// The “after” hook. By default, it passes through the response.
    async fn after(&self, resp: HttpResponse) -> HttpResponse {
        resp
    }
}
