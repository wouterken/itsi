use std::sync::{Arc, OnceLock};

use serde::Deserialize;

use crate::server::cache_store::CacheStore;

use super::{token_source::TokenSource, ErrorResponse, FromValue, MiddlewareLayer};

#[derive(Debug, Clone, Deserialize)]
pub struct RateLimit {
    pub requests: u64,
    pub seconds: u64,
    pub key: RateLimitKey,
    #[serde(skip_deserializing)]
    pub cache_store: OnceLock<Arc<dyn CacheStore>>,
    pub error_response: ErrorResponse,
}

#[derive(Debug, Clone, Deserialize)]
pub enum RateLimitKey {
    SocketAddress,
    Parameter(TokenSource),
}

impl MiddlewareLayer for RateLimit {}
impl FromValue for RateLimit {}
