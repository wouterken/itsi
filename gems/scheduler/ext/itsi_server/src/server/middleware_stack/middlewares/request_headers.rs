use std::collections::HashMap;

use super::{FromValue, MiddlewareLayer};
use crate::server::{
    itsi_service::RequestContext,
    types::{HttpRequest, HttpResponse},
};
use async_trait::async_trait;
use either::Either;
use http::HeaderName;
use magnus::error::Result;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct RequestHeaders {
    pub additions: HashMap<String, Vec<String>>,
    pub removals: Vec<String>,
}

#[async_trait]
impl MiddlewareLayer for RequestHeaders {
    async fn before(
        &self,
        mut req: HttpRequest,
        _: &mut RequestContext,
    ) -> Result<Either<HttpRequest, HttpResponse>> {
        let headers = req.headers_mut();
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
        Ok(Either::Left(req))
    }
}
impl FromValue for RequestHeaders {}
