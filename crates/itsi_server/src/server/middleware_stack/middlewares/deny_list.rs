use crate::{
    server::http_message_types::{HttpRequest, HttpResponse, RequestExt},
    services::itsi_http_service::HttpRequestContext,
};

use super::{token_source::TokenSource, ErrorResponse, FromValue, MiddlewareLayer};
use async_trait::async_trait;
use either::Either;
use itsi_error::ItsiError;
use magnus::error::Result;
use regex::RegexSet;
use serde::Deserialize;
use std::{collections::HashMap, sync::OnceLock};
use tracing::debug;

#[derive(Debug, Clone, Deserialize)]
pub struct DenyList {
    #[serde(skip_deserializing)]
    pub denied_ips: OnceLock<RegexSet>,
    pub denied_patterns: Vec<String>,
    pub trusted_proxies: HashMap<String, TokenSource>,
    #[serde(default = "forbidden_error_response")]
    pub error_response: ErrorResponse,
}

fn forbidden_error_response() -> ErrorResponse {
    ErrorResponse::forbidden()
}

#[async_trait]
impl MiddlewareLayer for DenyList {
    async fn initialize(&self) -> Result<()> {
        let denied_ips = RegexSet::new(&self.denied_patterns).map_err(ItsiError::new)?;
        self.denied_ips
            .set(denied_ips)
            .map_err(|e| ItsiError::new(format!("Failed to set allowed IPs: {:?}", e)))?;
        Ok(())
    }

    async fn before(
        &self,
        req: HttpRequest,
        context: &mut HttpRequestContext,
    ) -> Result<Either<HttpRequest, HttpResponse>> {
        let addr = if self.trusted_proxies.contains_key(&context.addr) {
            let source = self.trusted_proxies.get(&context.addr).unwrap();
            source.extract_token(&req).unwrap_or(&context.addr)
        } else {
            &context.addr
        };
        if let Some(denied_ips) = self.denied_ips.get() {
            if denied_ips.is_match(addr) {
                debug!(target: "middleware::deny_list", "IP address {} is not allowed", addr);
                return Ok(Either::Right(
                    self.error_response
                        .to_http_response(req.accept().into())
                        .await,
                ));
            }
        }
        Ok(Either::Left(req))
    }
}
impl FromValue for DenyList {}
