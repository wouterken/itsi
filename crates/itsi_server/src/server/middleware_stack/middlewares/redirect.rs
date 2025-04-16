use super::{string_rewrite::StringRewrite, FromValue, MiddlewareLayer};
use crate::{
    server::{
        http_message_types::{HttpRequest, HttpResponse},
        redirect_type::RedirectType,
    },
    services::itsi_http_service::HttpRequestContext,
};
use async_trait::async_trait;
use either::Either;
use http::Response;
use http_body_util::{combinators::BoxBody, Empty};
use magnus::error::Result;
use serde::Deserialize;
use tracing::debug;

#[derive(Debug, Clone, Deserialize)]
pub struct Redirect {
    pub to: StringRewrite,
    #[serde(default)]
    #[serde(rename(deserialize = "type"))]
    pub redirect_type: RedirectType,
}

#[async_trait]
impl MiddlewareLayer for Redirect {
    async fn before(
        &self,
        req: HttpRequest,
        context: &mut HttpRequestContext,
    ) -> Result<Either<HttpRequest, HttpResponse>> {
        Ok(Either::Right(self.redirect_response(&req, context)?))
    }
}

impl Redirect {
    pub fn redirect_response(
        &self,
        req: &HttpRequest,
        context: &mut HttpRequestContext,
    ) -> Result<HttpResponse> {
        let mut response = Response::new(BoxBody::new(Empty::new()));
        *response.status_mut() = self.redirect_type.status_code();
        let destination = self.to.rewrite_request(req, context).parse().map_err(|e| {
            magnus::Error::new(
                magnus::exception::standard_error(),
                format!("Invalid Rewrite String: {:?}: {}", self.to, e),
            )
        })?;
        debug!(target: "middleware::redirect", "Redirecting to {:?}", destination);
        response.headers_mut().append("Location", destination);
        Ok(response)
    }
}
impl FromValue for Redirect {}
