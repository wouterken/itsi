use std::collections::HashMap;

use crate::{
    server::http_message_types::{HttpRequest, HttpResponse},
    services::itsi_http_service::HttpRequestContext,
};

use super::{FromValue, MiddlewareLayer, StringRewrite};
use async_trait::async_trait;
use either::Either;
use http::HeaderName;
use magnus::error::Result;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct RequestHeaders {
    pub additions: HashMap<String, Vec<StringRewrite>>,
    pub removals: Vec<String>,
}
#[async_trait]
impl MiddlewareLayer for RequestHeaders {
    async fn before(
        &self,
        mut req: HttpRequest,
        context: &mut HttpRequestContext,
    ) -> Result<Either<HttpRequest, HttpResponse>> {
        let mut headers_to_add = Vec::new();

        for (header_name, header_values) in &self.additions {
            if let Ok(parsed_header_name) = header_name.parse::<HeaderName>() {
                for header_value in header_values {
                    if let Ok(parsed_header_value) =
                        header_value.rewrite_request(&req, context).parse()
                    {
                        headers_to_add.push((parsed_header_name.clone(), parsed_header_value));
                    }
                }
            }
        }

        let headers = req.headers_mut();

        for removal in &self.removals {
            headers.remove(removal);
        }

        for (name, value) in headers_to_add {
            headers.append(name, value);
        }

        Ok(Either::Left(req))
    }
}
impl FromValue for RequestHeaders {}
