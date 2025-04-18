use std::collections::HashMap;

use crate::{
    server::http_message_types::{HttpRequest, HttpResponse, RequestExt},
    services::{itsi_http_service::HttpRequestContext, password_hasher},
};

use super::{error_response::ErrorResponse, token_source::TokenSource, FromValue, MiddlewareLayer};

use async_trait::async_trait;
use either::Either;
use magnus::error::Result;
use serde::Deserialize;
use tracing::debug;

type PasswordHash = String;

/// A simple API key filter.
/// The API key can be given inside the header or a query string
/// Keys are validated against a list of allowed key values (Changing these requires a restart)
#[derive(Debug, Clone, Deserialize)]
pub struct AuthAPIKey {
    pub valid_keys: HashMap<String, PasswordHash>,
    pub key_id_source: Option<TokenSource>,
    pub token_source: TokenSource,
    #[serde(default = "unauthorized_error_response")]
    pub error_response: ErrorResponse,
}

fn unauthorized_error_response() -> ErrorResponse {
    ErrorResponse::unauthorized()
}

#[async_trait]
impl MiddlewareLayer for AuthAPIKey {
    async fn before(
        &self,
        req: HttpRequest,
        _context: &mut HttpRequestContext,
    ) -> Result<Either<HttpRequest, HttpResponse>> {
        if let Some(submitted_key) = match &self.token_source {
            TokenSource::Header { name, prefix } => {
                if let Some(header) = req.header(name) {
                    if let Some(prefix) = prefix {
                        Some(header.strip_prefix(prefix).unwrap_or("").trim_ascii())
                    } else {
                        Some(header.trim_ascii())
                    }
                } else {
                    None
                }
            }
            TokenSource::Query(query_name) => req.query_param(query_name),
        } {
            debug!(target: "middleware::auth_api_key", "API Key Retrieved. Anonymous {}", self.key_id_source.is_none());

            if let Some(key_id) = self.key_id_source.as_ref() {
                let key_id = match &key_id {
                    TokenSource::Header { name, prefix } => {
                        if let Some(header) = req.header(name) {
                            if let Some(prefix) = prefix {
                                Some(header.strip_prefix(prefix).unwrap_or("").trim_ascii())
                            } else {
                                Some(header.trim_ascii())
                            }
                        } else {
                            None
                        }
                    }
                    TokenSource::Query(query_name) => req.query_param(query_name),
                };
                debug!(target: "middleware::auth_api_key", "Key ID Retrieved");
                if let Some(hash) = key_id.and_then(|kid| self.valid_keys.get(kid)) {
                    debug!(target: "middleware::auth_api_key", "Key for ID found");
                    if password_hasher::verify_password_hash(submitted_key, hash).is_ok_and(|v| v) {
                        return Ok(Either::Left(req));
                    }
                }
            } else if self.valid_keys.values().any(|key| {
                password_hasher::verify_password_hash(submitted_key, key).is_ok_and(|v| v)
            }) {
                return Ok(Either::Left(req));
            }
        }

        debug!(target: "middleware::auth_api_key", "Failed to authenticate API key");
        Ok(Either::Right(
            self.error_response
                .to_http_response(req.accept().into())
                .await,
        ))
    }
}
impl FromValue for AuthAPIKey {}
