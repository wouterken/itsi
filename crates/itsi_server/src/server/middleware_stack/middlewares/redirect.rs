use crate::server::{
    itsi_service::RequestContext,
    types::{HttpRequest, HttpResponse},
};

use super::{string_rewrite::StringRewrite, FromValue, MiddlewareLayer};

use async_trait::async_trait;
use either::Either;
use http::{Response, StatusCode};
use http_body_util::{combinators::BoxBody, Empty};
use magnus::error::Result;
use serde::Deserialize;

/// A simple API key filter.
/// The API key can be given inside the header or a query string
/// Keys are validated against a list of allowed key values (Changing these requires a restart)
///
#[derive(Debug, Clone, Deserialize)]
pub struct Redirect {
    pub to: StringRewrite,
    #[serde(default)]
    #[serde(rename(deserialize = "type"))]
    pub redirect_type: RedirectType,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub enum RedirectType {
    #[serde(rename(deserialize = "permanent"))]
    #[default]
    Permanent,
    #[serde(rename(deserialize = "temporary"))]
    Temporary,
    #[serde(rename(deserialize = "found"))]
    Found,
    #[serde(rename(deserialize = "moved_permanently"))]
    MovedPermanently,
}

#[async_trait]
impl MiddlewareLayer for Redirect {
    async fn before(
        &self,
        req: HttpRequest,
        context: &mut RequestContext,
    ) -> Result<Either<HttpRequest, HttpResponse>> {
        Ok(Either::Right(self.redirect_response(&req, context)?))
    }
}

impl Redirect {
    pub fn redirect_response(
        &self,
        req: &HttpRequest,
        context: &mut RequestContext,
    ) -> Result<HttpResponse> {
        let mut response = Response::new(BoxBody::new(Empty::new()));
        *response.status_mut() = match self.redirect_type {
            RedirectType::Permanent => StatusCode::PERMANENT_REDIRECT,
            RedirectType::Temporary => StatusCode::TEMPORARY_REDIRECT,
            RedirectType::MovedPermanently => StatusCode::MOVED_PERMANENTLY,
            RedirectType::Found => StatusCode::FOUND,
        };
        response.headers_mut().append(
            "Location",
            self.to.rewrite_request(req, context).parse().map_err(|e| {
                magnus::Error::new(
                    magnus::exception::standard_error(),
                    format!("Invalid Rewrite String: {:?}: {}", self.to, e),
                )
            })?,
        );
        Ok(response)
    }
}
impl FromValue for Redirect {}
