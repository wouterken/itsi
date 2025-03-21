use std::collections::HashMap;

use super::{MiddlewareLayer, FromValue};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthBasic {
    pub realm: String,
    pub credential_pairs: HashMap<String, String>,
}

#[async_trait]
impl MiddlewareLayer for AuthBasic {}
impl FromValue for AuthBasic {}
