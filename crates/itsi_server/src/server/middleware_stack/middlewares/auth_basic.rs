use async_trait::async_trait;
use base64::{engine::general_purpose, Engine};
use bytes::Bytes;
use either::Either;
use http::{Response, StatusCode};
use http_body_util::{combinators::BoxBody, Full};
use magnus::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str;

use crate::server::{
    itsi_service::RequestContext,
    types::{HttpRequest, HttpResponse, RequestExt},
};

use super::{FromValue, MiddlewareLayer};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthBasic {
    pub realm: String,
    /// Maps usernames to passwords.
    pub credential_pairs: HashMap<String, String>,
}

impl AuthBasic {
    fn basic_auth_failed_response(&self) -> HttpResponse {
        Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .header(
                "WWW-Authenticate",
                format!("Basic realm=\"{}\"", self.realm),
            )
            .body(BoxBody::new(Full::new(Bytes::from("Unauthorized"))))
            .unwrap()
    }
}
#[async_trait]
impl MiddlewareLayer for AuthBasic {
    async fn before(
        &self,
        req: HttpRequest,
        _context: &mut RequestContext,
    ) -> Result<Either<HttpRequest, HttpResponse>> {
        // Retrieve the Authorization header.
        let auth_header = req.header("Authorization");

        if !auth_header.starts_with("Basic ") {
            return Ok(Either::Right(self.basic_auth_failed_response()));
        }

        let encoded_credentials = &auth_header["Basic ".len()..];
        let decoded = match general_purpose::STANDARD.decode(encoded_credentials) {
            Ok(bytes) => bytes,
            Err(_) => {
                return Ok(Either::Right(self.basic_auth_failed_response()));
            }
        };

        let decoded_str = match str::from_utf8(&decoded) {
            Ok(s) => s,
            Err(_) => {
                return Ok(Either::Right(self.basic_auth_failed_response()));
            }
        };

        let mut parts = decoded_str.splitn(2, ':');
        let username = parts.next().unwrap_or("");
        let password = parts.next().unwrap_or("");

        match self.credential_pairs.get(username) {
            Some(expected_password) if expected_password == password => Ok(Either::Left(req)),
            _ => {
                return Ok(Either::Right(self.basic_auth_failed_response()));
            }
        }
    }
}

impl FromValue for AuthBasic {}
