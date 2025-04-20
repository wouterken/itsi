use super::{token_source::TokenSource, ErrorResponse, FromValue, MiddlewareLayer};
use crate::{
    server::http_message_types::{HttpRequest, HttpResponse, RequestExt},
    services::itsi_http_service::HttpRequestContext,
};
use async_trait::async_trait;
use either::Either;
use itsi_error::ItsiError;
use magnus::error::Result;
use regex::RegexSet;
use serde::Deserialize;
use std::{collections::HashMap, sync::OnceLock};
use tracing::debug;

#[derive(Debug, Clone, Deserialize)]
pub struct AllowList {
    #[serde(skip_deserializing)]
    pub allowed_ips: OnceLock<RegexSet>,
    pub allowed_patterns: Vec<String>,
    pub trusted_proxies: HashMap<String, TokenSource>,
    #[serde(default = "forbidden_error_response")]
    pub error_response: ErrorResponse,
}

fn forbidden_error_response() -> ErrorResponse {
    ErrorResponse::forbidden()
}

#[async_trait]
impl MiddlewareLayer for AllowList {
    async fn initialize(&self) -> Result<()> {
        let allowed_ips = RegexSet::new(&self.allowed_patterns).map_err(ItsiError::new)?;
        self.allowed_ips
            .set(allowed_ips)
            .map_err(|e| ItsiError::new(format!("Failed to set allowed IPs: {:?}", e)))?;
        Ok(())
    }

    async fn before(
        &self,
        req: HttpRequest,
        context: &mut HttpRequestContext,
    ) -> Result<Either<HttpRequest, HttpResponse>> {
        if let Some(allowed_ips) = self.allowed_ips.get() {
            let addr = if self.trusted_proxies.contains_key(&context.addr) {
                let source = self.trusted_proxies.get(&context.addr).unwrap();
                source.extract_token(&req).unwrap_or(&context.addr)
            } else {
                &context.addr
            };
            if !allowed_ips.is_match(addr) {
                debug!(target: "middleware::allow_list", "IP address {} is not allowed", addr);
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
impl FromValue for AllowList {}
