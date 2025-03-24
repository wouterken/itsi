use super::error_response::ErrorResponse;
use super::{token_source::TokenSource, FromValue, MiddlewareLayer};
use crate::server::{
    itsi_service::RequestContext,
    types::{HttpRequest, HttpResponse, RequestExt},
};
use async_trait::async_trait;
use base64::{engine::general_purpose, Engine};
use either::Either;
use itsi_error::ItsiError;
use jwt_simple::{
    claims::{self, JWTClaims, NoCustomClaims},
    prelude::{
        ECDSAP256PublicKeyLike, ECDSAP384PublicKeyLike, ES256PublicKey, ES384PublicKey, HS256Key,
        HS384Key, HS512Key, MACLike, PS256PublicKey, PS384PublicKey, PS512PublicKey,
        RS256PublicKey, RS384PublicKey, RS512PublicKey, RSAPublicKeyLike,
    },
    token::Token,
};
use magnus::error::Result;
use serde::Deserialize;
use std::str;
use std::{
    collections::{HashMap, HashSet},
    sync::OnceLock,
};

#[derive(Debug, Clone, Deserialize)]
pub struct AuthJwt {
    pub token_source: TokenSource,
    pub verifiers: HashMap<JwtAlgorithm, Vec<String>>,
    #[serde(skip_deserializing)]
    pub keys: OnceLock<HashMap<JwtAlgorithm, Vec<JwtKey>>>,
    pub audiences: Option<HashSet<String>>,
    pub subjects: Option<HashSet<String>>,
    pub issuers: Option<HashSet<String>>,
    pub leeway: Option<u64>,
    pub error_response: ErrorResponse,
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

impl JwtAlgorithm {
    pub fn key_from(&self, base64: &str) -> Result<JwtKey> {
        let bytes = general_purpose::STANDARD
            .decode(base64)
            .map_err(ItsiError::default)?;

        match self {
            JwtAlgorithm::Hs256 => Ok(JwtKey::Hs256(HS256Key::from_bytes(&bytes))),
            JwtAlgorithm::Hs384 => Ok(JwtKey::Hs384(HS384Key::from_bytes(&bytes))),
            JwtAlgorithm::Hs512 => Ok(JwtKey::Hs512(HS512Key::from_bytes(&bytes))),
            JwtAlgorithm::Rs256 => Ok(RS256PublicKey::from_der(&bytes)
                .or_else(|_| {
                    RS256PublicKey::from_pem(
                        &String::from_utf8(bytes.clone()).map_err(ItsiError::default)?,
                    )
                })
                .map(JwtKey::Rs256)
                .map_err(ItsiError::default)?),
            JwtAlgorithm::Rs384 => Ok(RS384PublicKey::from_der(&bytes)
                .or_else(|_| {
                    RS384PublicKey::from_pem(
                        &String::from_utf8(bytes.clone()).map_err(ItsiError::default)?,
                    )
                })
                .map(JwtKey::Rs384)
                .map_err(ItsiError::default)?),
            JwtAlgorithm::Rs512 => Ok(RS512PublicKey::from_der(&bytes)
                .or_else(|_| {
                    RS512PublicKey::from_pem(
                        &String::from_utf8(bytes.clone()).map_err(ItsiError::default)?,
                    )
                })
                .map(JwtKey::Rs512)
                .map_err(ItsiError::default)?),
            JwtAlgorithm::Es256 => Ok(ES256PublicKey::from_der(&bytes)
                .or_else(|_| {
                    ES256PublicKey::from_pem(
                        &String::from_utf8(bytes.clone()).map_err(ItsiError::default)?,
                    )
                })
                .map(JwtKey::Es256)
                .map_err(ItsiError::default)?),
            JwtAlgorithm::Es384 => Ok(ES384PublicKey::from_der(&bytes)
                .or_else(|_| {
                    ES384PublicKey::from_pem(
                        &String::from_utf8(bytes.clone()).map_err(ItsiError::default)?,
                    )
                })
                .map(JwtKey::Es384)
                .map_err(ItsiError::default)?),
            JwtAlgorithm::Ps256 => Ok(PS256PublicKey::from_der(&bytes)
                .or_else(|_| {
                    PS256PublicKey::from_pem(
                        &String::from_utf8(bytes.clone()).map_err(ItsiError::default)?,
                    )
                })
                .map(JwtKey::Ps256)
                .map_err(ItsiError::default)?),
            JwtAlgorithm::Ps384 => Ok(PS384PublicKey::from_der(&bytes)
                .or_else(|_| {
                    PS384PublicKey::from_pem(
                        &String::from_utf8(bytes.clone()).map_err(ItsiError::default)?,
                    )
                })
                .map(JwtKey::Ps384)
                .map_err(ItsiError::default)?),
            JwtAlgorithm::Ps512 => Ok(PS512PublicKey::from_der(&bytes)
                .or_else(|_| {
                    PS512PublicKey::from_pem(
                        &String::from_utf8(bytes.clone()).map_err(ItsiError::default)?,
                    )
                })
                .map(JwtKey::Ps512)
                .map_err(ItsiError::default)?),
        }
    }
}

#[derive(Debug, Clone)]
pub enum JwtKey {
    Hs256(HS256Key),
    Hs384(HS384Key),
    Hs512(HS512Key),
    Rs256(RS256PublicKey),
    Rs384(RS384PublicKey),
    Rs512(RS512PublicKey),
    Es256(ES256PublicKey),
    Es384(ES384PublicKey),
    Ps256(PS256PublicKey),
    Ps384(PS384PublicKey),
    Ps512(PS512PublicKey),
}

impl TryFrom<&str> for JwtAlgorithm {
    type Error = itsi_error::ItsiError;

    fn try_from(value: &str) -> std::result::Result<Self, Self::Error> {
        match value.to_ascii_lowercase().as_str() {
            "hs256" => Ok(JwtAlgorithm::Hs256),
            "hs384" => Ok(JwtAlgorithm::Hs384),
            "hs512" => Ok(JwtAlgorithm::Hs512),
            "rs256" => Ok(JwtAlgorithm::Rs256),
            "rs384" => Ok(JwtAlgorithm::Rs384),
            "rs512" => Ok(JwtAlgorithm::Rs512),
            "es256" => Ok(JwtAlgorithm::Es256),
            "es384" => Ok(JwtAlgorithm::Es384),
            "ps256" => Ok(JwtAlgorithm::Ps256),
            "ps384" => Ok(JwtAlgorithm::Ps384),
            "ps512" => Ok(JwtAlgorithm::Ps512),
            _ => Err(itsi_error::ItsiError::UnsupportedProtocol(
                "Unsupported JWT Algorithm".to_string(),
            )),
        }
    }
}

impl JwtKey {
    pub fn verify(
        &self,
        token: &str,
    ) -> std::result::Result<JWTClaims<claims::NoCustomClaims>, jwt_simple::Error> {
        match self {
            JwtKey::Hs256(key) => key.verify_token::<NoCustomClaims>(token, None),
            JwtKey::Hs384(key) => key.verify_token::<NoCustomClaims>(token, None),
            JwtKey::Hs512(key) => key.verify_token::<NoCustomClaims>(token, None),
            JwtKey::Rs256(key) => key.verify_token::<NoCustomClaims>(token, None),
            JwtKey::Rs384(key) => key.verify_token::<NoCustomClaims>(token, None),
            JwtKey::Rs512(key) => key.verify_token::<NoCustomClaims>(token, None),
            JwtKey::Es256(key) => key.verify_token::<NoCustomClaims>(token, None),
            JwtKey::Es384(key) => key.verify_token::<NoCustomClaims>(token, None),
            JwtKey::Ps256(key) => key.verify_token::<NoCustomClaims>(token, None),
            JwtKey::Ps384(key) => key.verify_token::<NoCustomClaims>(token, None),
            JwtKey::Ps512(key) => key.verify_token::<NoCustomClaims>(token, None),
        }
    }
}

#[async_trait]
impl MiddlewareLayer for AuthJwt {
    async fn initialize(&self) -> Result<()> {
        let keys: HashMap<JwtAlgorithm, Vec<JwtKey>> = self
            .verifiers
            .iter()
            .map(|(algorithm, key_strings)| {
                let algo = algorithm.clone();
                let keys: Result<Vec<JwtKey>> = key_strings
                    .iter()
                    .map(|key_string| algorithm.key_from(key_string))
                    .collect();
                keys.map(|keys| (algo, keys))
            })
            .collect::<Result<HashMap<JwtAlgorithm, Vec<JwtKey>>>>()?;
        self.keys
            .set(keys)
            .map_err(|e| ItsiError::default(format!("Failed to set keys: {:?}", e)))?;
        Ok(())
    }

    async fn before(
        &self,
        req: HttpRequest,
        _context: &mut RequestContext,
    ) -> Result<Either<HttpRequest, HttpResponse>> {
        let token_str = match &self.token_source {
            TokenSource::Header { name, prefix } => {
                let header = req.header(name);
                if let Some(prefix) = prefix {
                    header.strip_prefix(prefix).unwrap_or("").trim_ascii()
                } else {
                    header.trim_ascii()
                }
            }
            TokenSource::Query(query_name) => req.query_param(query_name),
        };

        let token_meta = Token::decode_metadata(token_str);

        if token_meta.is_err() {
            return Ok(Either::Right(
                self.error_response.to_http_response(&req).await,
            ));
        }
        let token_meta: std::result::Result<JwtAlgorithm, ItsiError> =
            token_meta.unwrap().algorithm().try_into();
        if token_meta.is_err() {
            return Ok(Either::Right(
                self.error_response.to_http_response(&req).await,
            ));
        }
        let algorithm = token_meta.unwrap();

        if !self.verifiers.contains_key(&algorithm) {
            return Ok(Either::Right(
                self.error_response.to_http_response(&req).await,
            ));
        }

        let keys = self.keys.get().unwrap().get(&algorithm).unwrap();

        let verified_claims = keys.iter().find_map(|key| key.verify(token_str).ok());
        if verified_claims.is_none() {
            return Ok(Either::Right(
                self.error_response.to_http_response(&req).await,
            ));
        }

        let claims = verified_claims.unwrap();

        if let Some(expected_audiences) = &self.audiences {
            // The aud claim may be a string or an array.
            if let Some(audiences) = &claims.audiences {
                if !audiences.contains(expected_audiences) {
                    return Ok(Either::Right(
                        self.error_response.to_http_response(&req).await,
                    ));
                }
            }
        }

        if let Some(expected_subjects) = &self.subjects {
            // The aud claim may be a string or an array.
            if let Some(subject) = &claims.subject {
                if !expected_subjects.contains(subject) {
                    return Ok(Either::Right(
                        self.error_response.to_http_response(&req).await,
                    ));
                }
            }
        }

        if let Some(expected_issuers) = &self.issuers {
            // The aud claim may be a string or an array.
            if let Some(issuer) = &claims.issuer {
                if !expected_issuers.contains(issuer) {
                    return Ok(Either::Right(
                        self.error_response.to_http_response(&req).await,
                    ));
                }
            }
        }

        Ok(Either::Left(req))
    }
}

impl FromValue for AuthJwt {}
