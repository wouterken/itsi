use serde::{Deserialize, Serialize};

use crate::server::http_message_types::{HttpRequest, RequestExt};

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

impl TokenSource {
    pub fn extract_token<'req>(&self, req: &'req HttpRequest) -> Option<&'req str> {
        match self {
            TokenSource::Header { name, prefix } => req.headers().get(name).and_then(|value| {
                value.to_str().ok().and_then(|value| {
                    if let Some(prefix) = prefix {
                        value.strip_prefix(prefix).map(|v| v.trim())
                    } else {
                        Some(value)
                    }
                })
            }),
            TokenSource::Query(query_name) => req.query_param(query_name),
        }
    }
}
