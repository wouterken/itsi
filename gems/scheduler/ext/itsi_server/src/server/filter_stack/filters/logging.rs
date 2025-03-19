use super::{FilterLayer, FromValue};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Logging {}

impl FilterLayer for Logging {}
impl FromValue for Logging {}
