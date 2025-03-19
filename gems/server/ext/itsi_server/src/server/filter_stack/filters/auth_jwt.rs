use super::{token_source::TokenSource, FilterLayer, FromValue};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthJwt {
    pub algorithm: String,                       // e.g., "HS256" or "RS256"
    pub allowed_algorithms: Option<Vec<String>>, // if multiple are allowed
    pub secret: Option<String>,                  // for symmetric signing
    pub public_key: Option<String>,              // for asymmetric verification
    pub jwks_url: Option<String>,                // for dynamic key retrieval

    pub issuer: Option<String>,
    pub audience: Option<Vec<String>>,
    pub subject: Option<String>,
    pub required_claims: Option<std::collections::HashMap<String, String>>,
    pub leeway: Option<u64>, // in seconds

    pub token_source: Option<TokenSource>, // custom enum: Header, Cookie, Query
    pub header_name: Option<String>,       // default "Authorization"
    pub token_prefix: Option<String>,      // default "Bearer "
    pub cookie_name: Option<String>,

    pub error_message: Option<String>,
    pub status_code: Option<u16>,
    pub enable_revocation_check: Option<bool>,
    pub custom_validator: Option<String>, // placeholder for function name or reference
}

#[async_trait]
impl FilterLayer for AuthJwt {}
impl FromValue for AuthJwt {}
