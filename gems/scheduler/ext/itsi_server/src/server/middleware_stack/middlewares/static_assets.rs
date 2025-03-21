use super::{MiddlewareLayer, FromValue};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaticAssets {}

impl MiddlewareLayer for StaticAssets {}
impl FromValue for StaticAssets {}
