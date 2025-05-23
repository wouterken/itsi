use crate::{
    server::http_message_types::{HttpRequest, HttpResponse, RequestExt},
    services::itsi_http_service::HttpRequestContext,
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
    pub limit_bytes: usize,
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
        context: &mut HttpRequestContext,
    ) -> Result<Either<HttpRequest, HttpResponse>> {
        req.body().limit.store(self.limit_bytes, Ordering::Relaxed);
        context.set_response_format(req.accept().into());
        Ok(Either::Left(req))
    }

    async fn after(&self, resp: HttpResponse, context: &mut HttpRequestContext) -> HttpResponse {
        if resp.status() == StatusCode::PAYLOAD_TOO_LARGE {
            self.error_response
                .to_http_response(*context.response_format())
                .await
        } else {
            resp
        }
    }
}
impl FromValue for MaxBody {}
