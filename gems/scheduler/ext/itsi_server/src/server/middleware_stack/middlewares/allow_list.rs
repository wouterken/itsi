use super::{ErrorResponse, FromValue, MiddlewareLayer};
use crate::server::{
    itsi_service::RequestContext,
    types::{HttpRequest, HttpResponse},
};
use async_trait::async_trait;
use either::Either;
use itsi_error::ItsiError;
use magnus::error::Result;
use regex::RegexSet;
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Debug, Clone, Deserialize)]
pub struct AllowList {
    #[serde(skip_deserializing)]
    pub allowed_ips: OnceLock<RegexSet>,
    pub allowed_patterns: Vec<String>,
    pub error_response: ErrorResponse,
}

#[async_trait]
impl MiddlewareLayer for AllowList {
    async fn initialize(&self) -> Result<()> {
        let allowed_ips = RegexSet::new(&self.allowed_patterns).map_err(ItsiError::default)?;
        self.allowed_ips
            .set(allowed_ips)
            .map_err(|e| ItsiError::default(format!("Failed to set allowed IPs: {:?}", e)))?;
        Ok(())
    }

    async fn before(
        &self,
        req: HttpRequest,
        context: &mut RequestContext,
    ) -> Result<Either<HttpRequest, HttpResponse>> {
        if let Some(allowed_ips) = self.allowed_ips.get() {
            if !allowed_ips.is_match(&context.addr) {
                return Ok(Either::Right(
                    self.error_response.to_http_response(&req).await,
                ));
            }
        }
        Ok(Either::Left(req))
    }
}
impl FromValue for AllowList {}
