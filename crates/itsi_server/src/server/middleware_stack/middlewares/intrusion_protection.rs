use regex::RegexSet;
use serde::Deserialize;
use std::{
    collections::HashMap,
    sync::{Arc, OnceLock},
};

use crate::server::cache_store::CacheStore;

use super::{token_source::TokenSource, ErrorResponse, FromValue, MiddlewareLayer};

#[derive(Debug, Clone, Deserialize)]
pub struct IntrusionProtection {
    #[serde(skip_deserializing)]
    pub banned_url_pattern_matcher: OnceLock<RegexSet>,
    pub banned_url_patterns: Vec<String>,
    #[serde(skip_deserializing)]
    pub banned_header_pattern_matchers: OnceLock<HashMap<String, RegexSet>>,
    pub banned_header_patterns: HashMap<String, Vec<String>>,
    pub banned_time_seconds: u64,
    #[serde(skip_deserializing)]
    pub cache_store: OnceLock<Arc<dyn CacheStore>>,
    pub error_response: ErrorResponse,
}

#[derive(Debug, Clone, Deserialize)]
pub enum RateLimitKey {
    SocketAddress,
    Parameter(TokenSource),
}

impl MiddlewareLayer for IntrusionProtection {}
impl FromValue for IntrusionProtection {}
