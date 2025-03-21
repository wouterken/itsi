use super::{MiddlewareLayer, FromValue};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Logging {}

impl MiddlewareLayer for Logging {}
impl FromValue for Logging {}
