use std::collections::HashMap;

use super::{FilterLayer, FromValue};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthBasic {
    pub realm: String,
    pub credential_pairs: HashMap<String, String>,
}

#[async_trait]
impl FilterLayer for AuthBasic {}
impl FromValue for AuthBasic {}
