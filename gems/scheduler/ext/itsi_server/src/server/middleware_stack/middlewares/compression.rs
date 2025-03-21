use serde::{Deserialize, Serialize};

use super::{MiddlewareLayer, FromValue};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Compression {}

impl MiddlewareLayer for Compression {}
impl FromValue for Compression {}
