use super::{FilterLayer, FromValue};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Endpoint {}

impl FilterLayer for Endpoint {}
impl FromValue for Endpoint {}
