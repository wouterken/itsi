use super::{token_source::TokenSource, ErrorResponse, FromValue, MiddlewareLayer};
use crate::server::http_message_types::{HttpRequest, HttpResponse, RequestExt};
use crate::services::itsi_http_service::HttpRequestContext;
use crate::services::rate_limiter::{
    create_rate_limit_key, get_rate_limiter, RateLimitError, RateLimiter, RateLimiterConfig,
};
use async_trait::async_trait;
use either::Either;
use magnus::error::Result;
use serde::Deserialize;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

#[derive(Debug, Clone, Deserialize)]
pub struct RateLimit {
    pub requests: u64,
    pub seconds: u64,
    pub key: RateLimitKey,
    #[serde(skip_deserializing)]
    pub rate_limiter: OnceLock<Arc<dyn RateLimiter>>,
    pub store_config: RateLimiterConfig,
    #[serde(default = "too_many_requests_error_response")]
    pub error_response: ErrorResponse,
}

fn too_many_requests_error_response() -> ErrorResponse {
    ErrorResponse::too_many_requests()
}

#[derive(Debug, Clone, Deserialize)]
pub enum RateLimitKey {
    #[serde(rename(deserialize = "address"))]
    SocketAddress,
    #[serde(rename(deserialize = "parameter"))]
    Parameter(TokenSource),
}

#[async_trait]
impl MiddlewareLayer for RateLimit {
    async fn initialize(&self) -> Result<()> {
        // Instantiate our rate limiter based on the rate limit config here.
        // This will automatically fall back to in-memory if Redis fails
        if let Ok(limiter) = get_rate_limiter(&self.store_config).await {
            let _ = self.rate_limiter.set(limiter);
        }
        Ok(())
    }

    async fn before(
        &self,
        req: HttpRequest,
        context: &mut HttpRequestContext,
    ) -> Result<Either<HttpRequest, HttpResponse>> {
        // Get the key to rate limit on
        let key_value = match &self.key {
            RateLimitKey::SocketAddress => {
                // Use the socket address from the context
                &context.addr
            }
            RateLimitKey::Parameter(token_source) => {
                match token_source {
                    TokenSource::Header { name, prefix } => {
                        if let Some(header) = req.header(name) {
                            if let Some(prefix) = prefix {
                                header.strip_prefix(prefix).unwrap_or("").trim_ascii()
                            } else {
                                header.trim_ascii()
                            }
                        } else {
                            // If no token is found, skip rate limiting
                            tracing::warn!("No token found in header for rate limiting");
                            return Ok(Either::Left(req));
                        }
                    }
                    TokenSource::Query(query_name) => {
                        if let Some(value) = req.query_param(query_name) {
                            value
                        } else {
                            // If no token is found, skip rate limiting
                            tracing::warn!("No token found in query for rate limiting");
                            return Ok(Either::Left(req));
                        }
                    }
                }
            }
        };

        // Create a rate limit key
        let rate_limit_key = create_rate_limit_key(key_value, req.uri().path());

        // Get the rate limiter
        if let Some(limiter) = self.rate_limiter.get() {
            // Check if rate limit is exceeded
            let timeout = Duration::from_secs(self.seconds);
            let limit = self.requests;

            match limiter.check_limit(&rate_limit_key, limit, timeout).await {
                Ok(_) => Ok(Either::Left(req)),
                Err(RateLimitError::RateLimitExceeded { limit, ttl, .. }) => {
                    let mut response = self
                        .error_response
                        .to_http_response(req.accept().into())
                        .await;
                    response
                        .headers_mut()
                        .insert("X-RateLimit-Limit", limit.to_string().parse().unwrap());
                    response
                        .headers_mut()
                        .insert("X-RateLimit-Remaining", "0".parse().unwrap());
                    response
                        .headers_mut()
                        .insert("X-RateLimit-Reset", ttl.to_string().parse().unwrap());
                    response
                        .headers_mut()
                        .insert("Retry-After", ttl.to_string().parse().unwrap());
                    Ok(Either::Right(response))
                }
                Err(e) => {
                    // Other error, log and allow request (fail open)
                    tracing::error!("Rate limiter error: {:?}", e);
                    Ok(Either::Left(req))
                }
            }
        } else {
            // If rate limiter is not initialized, allow request
            tracing::warn!("Rate limiter not initialized");
            Ok(Either::Left(req))
        }
    }
}
impl FromValue for RateLimit {}
