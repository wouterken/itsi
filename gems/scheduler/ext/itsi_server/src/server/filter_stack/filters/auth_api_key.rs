use crate::server::{
    itsi_service::ItsiService,
    types::{HttpRequest, HttpResponse, RequestExt},
};

use super::{error_response::ErrorResponse, token_source::TokenSource, FilterLayer, FromValue};

use async_trait::async_trait;
use either::Either;
use magnus::error::Result;
use serde::Deserialize;

/// A simple API key filter.
/// The API key can be given inside the header or a query string
/// Keys are validated against a list of allowed key values (Changing these requires a restart)
///
#[derive(Debug, Clone, Deserialize)]
pub struct AuthAPIKey {
    pub valid_keys: Vec<String>,
    pub token_source: TokenSource,
    pub error_response: ErrorResponse,
}

#[async_trait]
impl FilterLayer for AuthAPIKey {
    async fn before(
        &self,
        req: HttpRequest,
        _context: &ItsiService,
    ) -> Result<Either<HttpRequest, HttpResponse>> {
        let submitted_value = match &self.token_source {
            TokenSource::Header(header_name) => req.header(header_name),
            TokenSource::Query(query_name) => req.query_param(query_name),
        };
        if !self.valid_keys.iter().any(|key| key == submitted_value) {
            Ok(Either::Right(
                self.error_response.to_http_response(&req).await,
            ))
        } else {
            Ok(Either::Left(req))
        }
    }
}
impl FromValue for AuthAPIKey {}
