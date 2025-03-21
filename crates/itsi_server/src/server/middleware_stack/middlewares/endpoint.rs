use super::{MiddlewareLayer, FromValue};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Endpoint {}

impl MiddlewareLayer for Endpoint {}
impl FromValue for Endpoint {}
