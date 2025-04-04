use crate::server::{
    itsi_service::RequestContext,
    types::{HttpRequest, HttpResponse, RequestExt},
};

use super::{ErrorResponse, FromValue, MiddlewareLayer};
use async_trait::async_trait;
use either::Either;
use http::StatusCode;
use magnus::error::Result;
use serde::Deserialize;
use std::sync::atomic::Ordering;

#[derive(Debug, Clone, Deserialize)]
pub struct MaxBody {
    pub max_size: usize,
    #[serde(default = "payload_too_large_error_response")]
    pub error_response: ErrorResponse,
}

fn payload_too_large_error_response() -> ErrorResponse {
    ErrorResponse::payload_too_large()
}

#[async_trait]
impl MiddlewareLayer for MaxBody {
    async fn before(
        &self,
        req: HttpRequest,
        context: &mut RequestContext,
    ) -> Result<Either<HttpRequest, HttpResponse>> {
        req.body().limit.store(self.max_size, Ordering::Relaxed);
        context.set_response_format(req.accept().into());
        Ok(Either::Left(req))
    }

    async fn after(&self, resp: HttpResponse, context: &mut RequestContext) -> HttpResponse {
        if resp.status() == StatusCode::PAYLOAD_TOO_LARGE {
            self.error_response
                .to_http_response(context.response_format().clone())
                .await
        } else {
            resp
        }
    }
}
impl FromValue for MaxBody {}
