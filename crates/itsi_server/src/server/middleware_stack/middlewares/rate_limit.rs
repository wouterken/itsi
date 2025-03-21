use serde::{Deserialize, Serialize};

use super::{MiddlewareLayer, FromValue};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimit {
    pub max_requests: u64,
    pub window_seconds: u64,
    pub burst_capacity: Option<u64>,
    pub strategy: Option<String>,      // e.g., "token_bucket"
    pub key_extractor: Option<String>, // e.g., "ip", "api_key"
    pub error_message: Option<String>,
    pub status_code: Option<u16>,
}

impl MiddlewareLayer for RateLimit {}
impl FromValue for RateLimit {}
