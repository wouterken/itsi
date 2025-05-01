use super::{token_source::TokenSource, ErrorResponse, FromValue, MiddlewareLayer};
use crate::server::http_message_types::{HttpRequest, HttpResponse, RequestExt};
use crate::services::itsi_http_service::HttpRequestContext;
use crate::services::rate_limiter::{
    create_rate_limit_key, get_rate_limiter, RateLimitError, RateLimiter, RateLimiterConfig,
};
use async_trait::async_trait;
use either::Either;
use http::{HeaderName, HeaderValue};
use magnus::error::Result;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tracing::{debug, error, warn};

#[derive(Debug, Clone, Deserialize)]
pub struct RateLimit {
    pub requests: u64,
    pub seconds: u64,
    pub key: RateLimitKey,
    #[serde(skip_deserializing)]
    pub rate_limiter: OnceLock<Arc<dyn RateLimiter>>,
    pub store_config: RateLimiterConfig,
    pub trusted_proxies: HashMap<String, TokenSource>,
    #[serde(default = "too_many_requests_error_response")]
    pub error_response: ErrorResponse,
    #[serde(skip)]
    pub limit_header_value: OnceLock<HeaderValue>,
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

static X_RATELIMIT_LIMIT: HeaderName = HeaderName::from_static("x-ratelimit-limit");
static X_RATELIMIT_REMAINING: HeaderName = HeaderName::from_static("x-ratelimit-remaining");
static X_RATELIMIT_RESET: HeaderName = HeaderName::from_static("x-ratelimit-reset");
static RETRY_AFTER: HeaderName = HeaderName::from_static("retry-after");
static ZERO_VALUE: HeaderValue = HeaderValue::from_static("0");

#[async_trait]
impl MiddlewareLayer for RateLimit {
    async fn initialize(&self) -> Result<()> {
        // Instantiate our rate limiter based on the rate limit config here.
        // This will automatically fall back to in-memory if Redis fails
        if let Ok(limiter) = get_rate_limiter(&self.store_config).await {
            let _ = self.rate_limiter.set(limiter);
        }
        self.limit_header_value
            .set(self.requests.to_string().parse().unwrap())
            .ok();
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
                if let Some(source) = self.trusted_proxies.get(&context.addr) {
                    source.extract_token(&req).unwrap_or(&context.addr)
                } else {
                    &context.addr
                }
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
                            warn!("No token found in header for rate limiting");
                            return Ok(Either::Left(req));
                        }
                    }
                    TokenSource::Query(query_name) => {
                        if let Some(value) = req.query_param(query_name) {
                            value
                        } else {
                            // If no token is found, skip rate limiting
                            warn!("No token found in query for rate limiting");
                            return Ok(Either::Left(req));
                        }
                    }
                }
            }
        };

        // Create a rate limit key
        let rate_limit_key = create_rate_limit_key(key_value, req.uri().path());

        debug!(target: "middleware::rate_limit", "Rate limit key: {}", rate_limit_key);
        // Get the rate limiter
        if let Some(limiter) = self.rate_limiter.get() {
            // Check if rate limit is exceeded
            let timeout = Duration::from_secs(self.seconds);
            let limit = self.requests;

            match limiter.check_limit(&rate_limit_key, limit, timeout).await {
                Ok(_) => {
                    debug!(target: "middleware::rate_limit", "Rate limit not exceeded");
                    Ok(Either::Left(req))
                }
                Err(RateLimitError::RateLimitExceeded { limit, ttl, .. }) => {
                    debug!(target: "middleware::rate_limit", "Rate limit exceeded. Limit: {}, ttl: {}", limit, ttl);
                    let mut response = self
                        .error_response
                        .to_http_response(req.accept().into())
                        .await;
                    let ttl_header_value: HeaderValue = ttl.to_string().parse().unwrap();
                    response.headers_mut().insert(
                        X_RATELIMIT_LIMIT.clone(),
                        self.limit_header_value.get().unwrap().clone(),
                    );
                    response
                        .headers_mut()
                        .insert(X_RATELIMIT_REMAINING.clone(), ZERO_VALUE.clone());
                    response
                        .headers_mut()
                        .insert(X_RATELIMIT_RESET.clone(), ttl_header_value.clone());
                    response
                        .headers_mut()
                        .insert(RETRY_AFTER.clone(), ttl_header_value);
                    Ok(Either::Right(response))
                }
                Err(e) => {
                    error!("Rate limiter error: {:?}", e);
                    Ok(Either::Left(req))
                }
            }
        } else {
            warn!("Rate limiter not initialized");
            Ok(Either::Left(req))
        }
    }
}
impl FromValue for RateLimit {}
