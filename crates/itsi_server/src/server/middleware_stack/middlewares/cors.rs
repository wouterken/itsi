use serde::{Deserialize, Serialize};

use super::{MiddlewareLayer, FromValue};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cors {
    pub allowed_origins: Vec<String>,
    pub allowed_methods: Vec<String>,
    pub allowed_headers: Vec<String>,
    pub exposed_headers: Vec<String>,
    pub allow_credentials: bool,
    pub max_age: Option<u64>,
}

impl MiddlewareLayer for Cors {}
impl FromValue for Cors {}
