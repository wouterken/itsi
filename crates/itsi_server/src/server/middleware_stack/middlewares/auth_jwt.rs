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
use tracing::{error, info};

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
            // For HMAC algorithms, use the secret directly.
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
enum Audience {
    Single(String),
    Multiple(Vec<String>),
}

#[derive(Debug, Deserialize)]
struct Claims {
    // Here we assume the token includes an expiration.
    #[allow(dead_code)]
    exp: usize,
    // The audience claim may be a single string or an array.
    aud: Option<Audience>,
    sub: Option<String>,
    iss: Option<String>,
}

#[async_trait]
impl MiddlewareLayer for AuthJwt {
    async fn initialize(&self) -> Result<()> {
        let keys: HashMap<JwtAlgorithm, Vec<DecodingKey>> = self
            .verifiers
            .iter()
            .map(|(algorithm, key_strings)| {
                let algo = algorithm.clone();
                let keys: itsi_error::Result<Vec<DecodingKey>> = key_strings
                    .iter()
                    .map(|key_string| algorithm.key_from(key_string))
                    .collect();
                keys.map(|keys| (algo, keys))
            })
            .collect::<itsi_error::Result<HashMap<JwtAlgorithm, Vec<DecodingKey>>>>()?;
        self.keys
            .set(keys)
            .map_err(|_| ItsiError::new("Failed to set keys"))?;
        Ok(())
    }

    async fn before(
        &self,
        req: HttpRequest,
        _context: &mut HttpRequestContext,
    ) -> Result<Either<HttpRequest, HttpResponse>> {
        // Retrieve the JWT token from either a header or a query parameter.
        let token_str = match &self.token_source {
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

        info!("Token str is {:?}", token_str);
        if token_str.is_none() {
            return Ok(Either::Right(
                self.error_response
                    .to_http_response(req.accept().into())
                    .await,
            ));
        }
        let token_str = token_str.unwrap();

        info!("Token str is {:?}", token_str);
        // Use jsonwebtoken's decode_header to inspect the token and determine its algorithm.
        let header =
            decode_header(token_str).map_err(|_| ItsiError::new("Invalid token header"))?;
        info!("Header is {:?}", header);
        let alg: JwtAlgorithm = header.alg.into();

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

        info!("Validation is {:?}", validation);
        if let Some(leeway) = self.leeway {
            validation.leeway = leeway;
        }
        // (Optional) You could set expected issuer or audience on `validation` here.

        info!("Keys are {:?}", keys.len());
        // Try verifying the token using each key until one succeeds.
        let token_data: Option<TokenData<Claims>> =
            keys.iter()
                .find_map(|key| match decode::<Claims>(token_str, key, &validation) {
                    Ok(data) => Some(data),
                    Err(e) => {
                        error!("Token validation failed: {:?}", e);
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

        info!("Got past token verification");

        let claims = token_data.claims;

        // Verify expected audiences.
        if let Some(expected_audiences) = &self.audiences {
            if let Some(aud) = &claims.aud {
                let token_auds: HashSet<String> = match aud {
                    Audience::Single(s) => [s.clone()].into_iter().collect(),
                    Audience::Multiple(v) => v.iter().cloned().collect(),
                };
                if expected_audiences.is_disjoint(&token_auds) {
                    return Ok(Either::Right(
                        self.error_response
                            .to_http_response(req.accept().into())
                            .await,
                    ));
                }
            }
        }

        // Verify expected subject.
        if let Some(expected_subjects) = &self.subjects {
            if let Some(sub) = &claims.sub {
                if !expected_subjects.contains(sub) {
                    return Ok(Either::Right(
                        self.error_response
                            .to_http_response(req.accept().into())
                            .await,
                    ));
                }
            }
        }

        // Verify expected issuer.
        if let Some(expected_issuers) = &self.issuers {
            if let Some(iss) = &claims.iss {
                if !expected_issuers.contains(iss) {
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
