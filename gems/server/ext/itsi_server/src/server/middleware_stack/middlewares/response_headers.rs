use std::collections::HashMap;

use super::{FromValue, MiddlewareLayer};
use crate::server::{itsi_service::RequestContext, types::HttpResponse};
use async_trait::async_trait;
use http::HeaderName;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct ResponseHeaders {
    pub additions: HashMap<String, Vec<String>>,
    pub removals: Vec<String>,
}

#[async_trait]
impl MiddlewareLayer for ResponseHeaders {
    async fn after(&self, mut resp: HttpResponse, _: &mut RequestContext) -> HttpResponse {
        let headers = resp.headers_mut();
        for removal in &self.removals {
            headers.remove(removal);
        }
        for (header_name, header_values) in &self.additions {
            for header_value in header_values {
                if let Ok(parsed_header_name) = header_name.parse::<HeaderName>() {
                    if let Ok(parsed_header_value) = header_value.parse() {
                        headers.append(parsed_header_name, parsed_header_value);
                    }
                }
            }
        }
        resp
    }
}
impl FromValue for ResponseHeaders {}
