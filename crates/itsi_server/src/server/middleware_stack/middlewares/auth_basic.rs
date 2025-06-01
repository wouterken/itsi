use async_trait::async_trait;
use base64::{engine::general_purpose, Engine};
use bytes::Bytes;
use either::Either;
use http::{Response, StatusCode};
use magnus::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str;
use tracing::debug;

use crate::{
    server::http_message_types::{HttpBody, HttpRequest, HttpResponse, RequestExt},
    services::{itsi_http_service::HttpRequestContext, password_hasher::verify_password_hash},
};

use super::{FromValue, MiddlewareLayer};

type PasswordHash = String;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthBasic {
    pub realm: String,
    /// Maps usernames to passwords.
    pub credential_pairs: HashMap<String, PasswordHash>,
}

impl AuthBasic {
    fn basic_auth_failed_response(&self) -> HttpResponse {
        Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .header(
                "WWW-Authenticate",
                format!("Basic realm=\"{}\"", self.realm),
            )
            .body(HttpBody::full(Bytes::from("Unauthorized")))
            .unwrap()
    }
}
#[async_trait]
impl MiddlewareLayer for AuthBasic {
    async fn before(
        &self,
        req: HttpRequest,
        _context: &mut HttpRequestContext,
    ) -> Result<Either<HttpRequest, HttpResponse>> {
        // Retrieve the Authorization header.
        let auth_header = req.header("Authorization");

        if !auth_header.is_some_and(|header| header.starts_with("Basic ")) {
            debug!(target: "middleware::auth_basic", "Basic auth failed. Authorization Header doesn't start with 'Basic '");
            return Ok(Either::Right(self.basic_auth_failed_response()));
        }

        let auth_header = auth_header.unwrap();

        let encoded_credentials = &auth_header["Basic ".len()..];
        let decoded = match general_purpose::STANDARD.decode(encoded_credentials) {
            Ok(bytes) => bytes,
            Err(_) => {
                debug!(target: "middleware::auth_basic", "Basic auth failed. Decoding failed");
                return Ok(Either::Right(self.basic_auth_failed_response()));
            }
        };

        let decoded_str = match str::from_utf8(&decoded) {
            Ok(s) => s,
            Err(_) => {
                debug!(target: "middleware::auth_basic", "Basic auth failed. Decoding failed");
                return Ok(Either::Right(self.basic_auth_failed_response()));
            }
        };

        let mut parts = decoded_str.splitn(2, ':');
        let username = parts.next().unwrap_or("");
        let password = parts.next().unwrap_or("");

        match self.credential_pairs.get(username) {
            Some(expected_password_hash) => {
                match verify_password_hash(password, expected_password_hash) {
                    Ok(true) => Ok(Either::Left(req)),
                    _ => Ok(Either::Right(self.basic_auth_failed_response())),
                }
            }
            None => {
                debug!(target: "middleware::auth_basic", "Basic auth failed. Username {} not found", username);
                Ok(Either::Right(self.basic_auth_failed_response()))
            }
        }
    }
}

impl FromValue for AuthBasic {}
