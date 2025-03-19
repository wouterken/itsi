use super::{FilterLayer, FromValue};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaticAssets {}

impl FilterLayer for StaticAssets {}
impl FromValue for StaticAssets {}
