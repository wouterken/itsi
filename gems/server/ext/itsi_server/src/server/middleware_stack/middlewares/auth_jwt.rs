use async_trait::async_trait;
use either::Either;
use jwt::Header;
use jwt::{Token, VerifyWithKey};
use magnus::error::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::str;

use crate::server::{
    itsi_service::RequestContext,
    types::{HttpRequest, HttpResponse, RequestExt},
};

use super::{token_source::TokenSource, FromValue, MiddlewareLayer};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthJwt {
    pub algorithm: String,                       // e.g., "HS256"
    pub allowed_algorithms: Option<Vec<String>>, // not used in this example but available for future checks
    pub secret: Option<String>,                  // for symmetric signing
    pub public_key: Option<String>,              // for asymmetric verification
    pub jwks_url: Option<String>,                // for dynamic key retrieval (not implemented here)

    pub issuer: Option<String>,
    pub audience: Option<Vec<String>>,
    pub subject: Option<String>,
    pub required_claims: Option<HashMap<String, String>>,
    pub leeway: Option<u64>, // in seconds, not used in this example

    pub token_source: Option<TokenSource>, // custom enum: Header, Cookie, Query
    pub header_name: Option<String>,       // default "Authorization"
    pub token_prefix: Option<String>,      // default "Bearer "
    pub cookie_name: Option<String>,

    pub error_message: Option<String>,
    pub status_code: Option<u16>,
    pub enable_revocation_check: Option<bool>,
    pub custom_validator: Option<String>, // placeholder for a custom validator reference
}

impl AuthJwt {
    fn jwt_failed_response(&self) -> HttpResponse {
        let status = self.status_code.unwrap_or(401);
        let msg = self
            .error_message
            .clone()
            .unwrap_or_else(|| "Unauthorized".to_string());
        todo!()
    }
}

#[async_trait]
impl MiddlewareLayer for AuthJwt {
    async fn before(
        &self,
        req: HttpRequest,
        _context: &mut RequestContext,
    ) -> Result<Either<HttpRequest, HttpResponse>> {
        // --- Token Extraction ---
        let token_str = match &self.token_source {
            Some(TokenSource::Header { name, prefix }) => {
                let header_name = name;
                let header = req.header(header_name);
                if let Some(prefix) = prefix {
                    if header.starts_with(prefix) {
                        header[prefix.len()..].trim().to_string()
                    } else {
                        header.to_string()
                    }
                } else {
                    header.to_string()
                }
            }
            Some(TokenSource::Query(query_name)) => req.query_param(query_name).to_owned(),
            None => {
                // Default: look for the Authorization header with "Bearer " prefix.
                let header = req.header("Authorization");
                if header.starts_with("Bearer ") {
                    header["Bearer ".len()..].trim().to_string()
                } else {
                    header.to_string()
                }
            }
        };

        if token_str.is_empty() {
            return Ok(Either::Right(self.jwt_failed_response()));
        }

        // --- Parsing and Algorithm Check ---
        // Parse token without verification so we can inspect its header.
        let token = Token::<Header, Value, _>::parse_unverified(&token_str).map_err(|e| {
            magnus::Error::new(
                magnus::exception::exception(),
                format!("Failed to parse JWT: {:?}", e),
            )
        })?;

        let algorithm_match = match token.header().algorithm {
            jwt::AlgorithmType::Hs256 => self.algorithm == "hs256",
            jwt::AlgorithmType::Hs384 => self.algorithm == "hs384",
            jwt::AlgorithmType::Hs512 => self.algorithm == "hs512",
            jwt::AlgorithmType::Rs256 => self.algorithm == "rs256",
            jwt::AlgorithmType::Rs384 => self.algorithm == "rs384",
            jwt::AlgorithmType::Rs512 => self.algorithm == "rs512",
            jwt::AlgorithmType::Es256 => self.algorithm == "es256",
            jwt::AlgorithmType::Es384 => self.algorithm == "es384",
            jwt::AlgorithmType::Es512 => self.algorithm == "es512",
            jwt::AlgorithmType::Ps256 => self.algorithm == "ps256",
            jwt::AlgorithmType::Ps384 => self.algorithm == "ps384",
            jwt::AlgorithmType::Ps512 => self.algorithm == "ps512",
            jwt::AlgorithmType::None => self.algorithm == "none",
        };

        todo!();
        // // Ensure the token's algorithm matches our configuration.
        // if token.header().algorithm.to_string() != self.algorithm {
        //     return Ok(Either::Right(self.jwt_failed_response()));
        // }

        // // --- Verification ---
        // let verified_token = if let Some(ref secret) = self.secret {
        //     // Symmetric verification.
        //     token.verify_with_key(secret).map_err(|e| {
        //         magnus::Error::new(
        //             magnus::exception::exception(),
        //             format!("JWT verification failed: {:?}", e),
        //         )
        //     })?
        // } else if let Some(ref public_key) = self.public_key {
        //     // Asymmetric verification.
        //     token.verify_with_key(public_key).map_err(|e| {
        //         magnus::Error::new(
        //             magnus::exception::exception(),
        //             format!("JWT verification failed: {:?}", e),
        //         )
        //     })?
        // } else {
        //     return Ok(Either::Right(self.jwt_failed_response()));
        // };

        // let claims = verified_token.claims();

        // // --- Claims Validation ---
        // if let Some(expected_issuer) = &self.issuer {
        //     if claims.get("iss").and_then(|v| v.as_str()) != Some(expected_issuer) {
        //         return Ok(Either::Right(self.jwt_failed_response()));
        //     }
        // }

        // if let Some(expected_audience) = &self.audience {
        //     // The aud claim may be a string or an array.
        //     if let Some(aud) = claims.get("aud").and_then(|v| v.as_str()) {
        //         if !expected_audience.contains(&aud.to_string()) {
        //             return Ok(Either::Right(self.jwt_failed_response()));
        //         }
        //     } else if let Some(aud_array) = claims.get("aud").and_then(|v| v.as_array()) {
        //         let aud_strs: Vec<String> = aud_array
        //             .iter()
        //             .filter_map(|v| v.as_str().map(|s| s.to_string()))
        //             .collect();
        //         if expected_audience.iter().all(|a| !aud_strs.contains(a)) {
        //             return Ok(Either::Right(self.jwt_failed_response()));
        //         }
        //     } else {
        //         return Ok(Either::Right(self.jwt_failed_response()));
        //     }
        // }

        // if let Some(expected_subject) = &self.subject {
        //     if claims.get("sub").and_then(|v| v.as_str()) != Some(expected_subject) {
        //         return Ok(Either::Right(self.jwt_failed_response()));
        //     }
        // }

        // if let Some(required_claims) = &self.required_claims {
        //     for (key, value) in required_claims {
        //         if claims.get(key).and_then(|v| v.as_str()) != Some(value) {
        //             return Ok(Either::Right(self.jwt_failed_response()));
        //         }
        //     }
        // }

        // // Optionally, implement revocation checks or call a custom validator here.

        // // Token verified and claims validated.
        // Ok(Either::Left(req))
    }
}

impl FromValue for AuthJwt {}
