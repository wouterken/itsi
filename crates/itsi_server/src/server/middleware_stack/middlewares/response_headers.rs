use std::collections::HashMap;

use super::{FromValue, MiddlewareLayer, StringRewrite};
use crate::{
    server::http_message_types::HttpResponse, services::itsi_http_service::HttpRequestContext,
};
use async_trait::async_trait;
use http::HeaderName;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct ResponseHeaders {
    pub additions: HashMap<String, Vec<StringRewrite>>,
    pub removals: Vec<String>,
}

#[async_trait]
impl MiddlewareLayer for ResponseHeaders {
    async fn after(
        &self,
        mut resp: HttpResponse,
        context: &mut HttpRequestContext,
    ) -> HttpResponse {
        let mut headers_to_add = Vec::new();

        for (header_name, header_values) in &self.additions {
            if let Ok(parsed_header_name) = header_name.parse::<HeaderName>() {
                for header_value in header_values {
                    if let Ok(parsed_header_value) =
                        header_value.rewrite_response(&resp, context).parse()
                    {
                        headers_to_add.push((parsed_header_name.clone(), parsed_header_value));
                    }
                }
            }
        }

        let headers = resp.headers_mut();

        for removal in &self.removals {
            headers.remove(removal);
        }

        for (name, value) in headers_to_add {
            headers.append(name, value);
        }

        resp
    }
}
impl FromValue for ResponseHeaders {}
