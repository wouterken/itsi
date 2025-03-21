use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TokenSource {
    #[serde(rename(deserialize = "header"))]
    Header {
        name: String,
        prefix: Option<String>,
    },
    #[serde(rename(deserialize = "query"))]
    Query(String),
}
