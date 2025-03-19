use serde::{Deserialize, Serialize};

use super::{FilterLayer, FromValue};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Compression {}

impl FilterLayer for Compression {}
impl FromValue for Compression {}
