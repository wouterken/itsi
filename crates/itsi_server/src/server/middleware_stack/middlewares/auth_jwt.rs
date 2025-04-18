use super::{error_response::ErrorResponse, token_source::TokenSource, FromValue, MiddlewareLayer};
use crate::{
    server::http_message_types::{HttpRequest, HttpResponse, RequestExt},
    services::itsi_http_service::HttpRequestContext,
};

use async_trait::async_trait;
use base64::{engine::general_purpose, Engine};
use derive_more::Debug;
use either::Either;
use itsi_error::ItsiError;
use jsonwebtoken::{
    decode, decode_header, Algorithm as JwtAlg, DecodingKey, TokenData, Validation,
};
use magnus::error::Result;
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    sync::OnceLock,
};
use tracing::debug;

#[derive(Debug, Clone, Deserialize)]
pub struct AuthJwt {
    pub token_source: TokenSource,
    // The verifiers map still holds base64-encoded key strings keyed by algorithm.
    pub verifiers: HashMap<JwtAlgorithm, Vec<String>>,
    // We now store jsonwebtoken’s DecodingKey in our OnceLock.
    #[serde(skip_deserializing)]
    #[debug(skip)]
    pub keys: OnceLock<HashMap<JwtAlgorithm, Vec<DecodingKey>>>,
    pub audiences: Option<HashSet<String>>,
    pub subjects: Option<HashSet<String>>,
    pub issuers: Option<HashSet<String>>,
    #[serde(skip_deserializing)]
    pub audience_vec: OnceLock<Option<Vec<String>>>,
    #[serde(skip_deserializing)]
    pub issuer_vec: OnceLock<Option<Vec<String>>>,
    pub leeway: Option<u64>,
    #[serde(default = "unauthorized_error_response")]
    pub error_response: ErrorResponse,
}

fn unauthorized_error_response() -> ErrorResponse {
    ErrorResponse::unauthorized()
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Hash)]
pub enum JwtAlgorithm {
    #[serde(rename(deserialize = "hs256"))]
    Hs256,
    #[serde(rename(deserialize = "hs384"))]
    Hs384,
    #[serde(rename(deserialize = "hs512"))]
    Hs512,
    #[serde(rename(deserialize = "rs256"))]
    Rs256,
    #[serde(rename(deserialize = "rs384"))]
    Rs384,
    #[serde(rename(deserialize = "rs512"))]
    Rs512,
    #[serde(rename(deserialize = "es256"))]
    Es256,
    #[serde(rename(deserialize = "es384"))]
    Es384,
    #[serde(rename(deserialize = "ps256"))]
    Ps256,
    #[serde(rename(deserialize = "ps384"))]
    Ps384,
    #[serde(rename(deserialize = "ps512"))]
    Ps512,
}

// Allow conversion from jsonwebtoken’s Algorithm to our JwtAlgorithm.
impl From<JwtAlg> for JwtAlgorithm {
    fn from(alg: JwtAlg) -> Self {
        match alg {
            JwtAlg::HS256 => JwtAlgorithm::Hs256,
            JwtAlg::HS384 => JwtAlgorithm::Hs384,
            JwtAlg::HS512 => JwtAlgorithm::Hs512,
            JwtAlg::RS256 => JwtAlgorithm::Rs256,
            JwtAlg::RS384 => JwtAlgorithm::Rs384,
            JwtAlg::RS512 => JwtAlgorithm::Rs512,
            JwtAlg::ES256 => JwtAlgorithm::Es256,
            JwtAlg::ES384 => JwtAlgorithm::Es384,
            JwtAlg::PS256 => JwtAlgorithm::Ps256,
            JwtAlg::PS384 => JwtAlgorithm::Ps384,
            JwtAlg::PS512 => JwtAlgorithm::Ps512,
            _ => panic!("Unsupported algorithm"),
        }
    }
}

impl JwtAlgorithm {
    /// Given a base64-encoded key string, decode and construct a jsonwebtoken::DecodingKey.
    pub fn key_from(&self, base64: &str) -> itsi_error::Result<DecodingKey> {
        match self {
            // For HMAC algorithms, expect a base64 encoded secret.
            JwtAlgorithm::Hs256 | JwtAlgorithm::Hs384 | JwtAlgorithm::Hs512 => {
                Ok(DecodingKey::from_secret(
                    &general_purpose::STANDARD
                        .decode(base64)
                        .map_err(ItsiError::new)?,
                ))
            }
            // For RSA (and PS) algorithms, expect a PEM-formatted key.
            JwtAlgorithm::Rs256
            | JwtAlgorithm::Rs384
            | JwtAlgorithm::Rs512
            | JwtAlgorithm::Ps256
            | JwtAlgorithm::Ps384
            | JwtAlgorithm::Ps512 => DecodingKey::from_rsa_pem(base64.trim_ascii().as_bytes())
                .map_err(|e| ItsiError::new(e.to_string())),
            // For ECDSA algorithms, expect a PEM-formatted key.
            JwtAlgorithm::Es256 | JwtAlgorithm::Es384 => {
                DecodingKey::from_ec_pem(base64.trim_ascii().as_bytes())
                    .map_err(|e| ItsiError::new(e.to_string()))
            }
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
#[allow(dead_code)]
enum Audience {
    Single(String),
    Multiple(Vec<String>),
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Claims {
    // Here we assume the token includes an expiration.
    exp: usize,
    // The audience claim may be a single string or an array.
    aud: Option<Audience>,
    sub: Option<String>,
    iss: Option<String>,
}

#[async_trait]
impl MiddlewareLayer for AuthJwt {
    async fn initialize(&self) -> Result<()> {
        debug!(
            target: "middleware::auth_jwt",
            "Instantiating auth_jwt with {} verifiers", self.verifiers.len()
        );

        let keys: HashMap<JwtAlgorithm, Vec<DecodingKey>> = self
            .verifiers
            .iter()
            .map(|(algorithm, key_strings)| {
                let algo = algorithm.clone();
                let keys: itsi_error::Result<Vec<DecodingKey>> = key_strings
                    .iter()
                    .map(|key_string| algorithm.key_from(key_string))
                    .inspect(|key_result| {
                        if key_result.is_err() {
                            debug!(
                                target: "middleware::auth_jwt",
                                "Failed to load key for algorithm {:?}", algorithm
                            )
                        } else {
                            debug!(
                                target: "middleware::auth_jwt",
                                "Loaded key for algorithm {:?}", algorithm
                            )
                        }
                    })
                    .collect();
                keys.map(|keys| (algo, keys))
            })
            .collect::<itsi_error::Result<HashMap<JwtAlgorithm, Vec<DecodingKey>>>>()?;

        self.keys
            .set(keys)
            .map_err(|_| ItsiError::new("Failed to set keys"))?;

        if let Some(audiences) = self.audiences.as_ref() {
            self.audience_vec
                .set(Some(audiences.iter().cloned().collect::<Vec<_>>()))
                .ok();
        }
        if let Some(issuers) = self.issuers.as_ref() {
            self.issuer_vec
                .set(Some(issuers.iter().cloned().collect::<Vec<_>>()))
                .ok();
        }
        Ok(())
    }

    async fn before(
        &self,
        req: HttpRequest,
        _: &mut HttpRequestContext,
    ) -> Result<Either<HttpRequest, HttpResponse>> {
        // Retrieve the JWT token from either a header or a query parameter.
        let token_str = match &self.token_source {
            TokenSource::Header { name, prefix } => {
                debug!(
                    target: "middleware::auth_jwt",
                    "Extracting JWT from header: {}, prefix: {:?}",
                    name, prefix
                );
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
            TokenSource::Query(query_name) => {
                debug!(
                  target: "middleware::auth_jwt",
                    "Extracting JWT from query parameter: {}",
                    query_name
                );
                req.query_param(query_name)
            }
        };

        if token_str.is_none() {
            debug!(
              target: "middleware::auth_jwt",
                "No JWT found in headers or query parameters"
            );
            return Ok(Either::Right(
                self.error_response
                    .to_http_response(req.accept().into())
                    .await,
            ));
        }
        let token_str = token_str.unwrap();
        let header = match decode_header(token_str) {
            Ok(header) => header,
            Err(_) => {
                debug!(target: "middleware::auth_jwt", "JWT decoding failed");
                return Ok(Either::Right(
                    self.error_response
                        .to_http_response(req.accept().into())
                        .await,
                ));
            }
        };

        let alg: JwtAlgorithm = header.alg.into();

        debug!(
          target: "middleware::auth_jwt",
            "Matched algorithm {:?}", alg
        );
        if !self.verifiers.contains_key(&alg) {
            return Ok(Either::Right(
                self.error_response
                    .to_http_response(req.accept().into())
                    .await,
            ));
        }
        let keys = self.keys.get().unwrap().get(&alg).unwrap();

        // Build validation based on the algorithm and optional leeway.
        let mut validation = Validation::new(match alg {
            JwtAlgorithm::Hs256 => JwtAlg::HS256,
            JwtAlgorithm::Hs384 => JwtAlg::HS384,
            JwtAlgorithm::Hs512 => JwtAlg::HS512,
            JwtAlgorithm::Rs256 => JwtAlg::RS256,
            JwtAlgorithm::Rs384 => JwtAlg::RS384,
            JwtAlgorithm::Rs512 => JwtAlg::RS512,
            JwtAlgorithm::Es256 => JwtAlg::ES256,
            JwtAlgorithm::Es384 => JwtAlg::ES384,
            JwtAlgorithm::Ps256 => JwtAlg::PS256,
            JwtAlgorithm::Ps384 => JwtAlg::PS384,
            JwtAlgorithm::Ps512 => JwtAlg::PS512,
        });

        if let Some(leeway) = self.leeway {
            validation.leeway = leeway;
        }

        if let Some(Some(auds)) = &self.audience_vec.get() {
            validation.set_audience(auds);
            validation.required_spec_claims.insert("aud".to_owned());
        } else {
            validation.validate_aud = false;
        }

        if let Some(Some(issuers)) = &self.issuer_vec.get() {
            validation.set_issuer(issuers);
            validation.required_spec_claims.insert("iss".to_owned());
        }

        if self.subjects.is_some() {
            validation.required_spec_claims.insert("sub".to_owned());
        }

        let token_data: Option<TokenData<Claims>> =
            keys.iter()
                .find_map(|key| match decode::<Claims>(token_str, key, &validation) {
                    Ok(data) => Some(data),
                    Err(e) => {
                        debug!("Token validation failed: {:?}", e);
                        None
                    }
                });

        let token_data = if let Some(data) = token_data {
            data
        } else {
            return Ok(Either::Right(
                self.error_response
                    .to_http_response(req.accept().into())
                    .await,
            ));
        };

        let claims = token_data.claims;

        if let Some(expected_subjects) = &self.subjects {
            if let Some(sub) = &claims.sub {
                if !expected_subjects.contains(sub) {
                    debug!(
                        target: "middleware::auth_jwt",
                        "SUB check failed, token_sub: {:?}, expected_subjects: {:?}",
                        sub, expected_subjects
                    );
                    return Ok(Either::Right(
                        self.error_response
                            .to_http_response(req.accept().into())
                            .await,
                    ));
                }
            }
        }

        Ok(Either::Left(req))
    }
}

impl FromValue for AuthJwt {}
