use crate::server::http_message_types::{HttpRequest, HttpResponse, RequestExt};
use crate::services::itsi_http_service::HttpRequestContext;
use crate::services::rate_limiter::{
    get_ban_manager, get_rate_limiter, BanManager, RateLimiter, RateLimiterConfig,
};

use super::{ErrorResponse, FromValue, MiddlewareLayer};

use async_trait::async_trait;
use either::Either;
use itsi_tracing::*;
use magnus::error::Result;
use regex::RegexSet;
use serde::Deserialize;
use std::time::Duration;
use std::{
    collections::HashMap,
    sync::{Arc, OnceLock},
};

#[derive(Debug, Clone, Deserialize)]
pub struct IntrusionProtection {
    #[serde(skip_deserializing)]
    pub banned_url_pattern_matcher: OnceLock<RegexSet>,
    #[serde(default)]
    pub banned_url_patterns: Vec<String>,
    #[serde(skip_deserializing)]
    pub banned_header_pattern_matchers: OnceLock<HashMap<String, RegexSet>>,
    #[serde(default)]
    pub banned_header_patterns: HashMap<String, Vec<String>>,
    pub banned_time_seconds: u64,
    #[serde(skip_deserializing)]
    pub rate_limiter: OnceLock<Arc<dyn RateLimiter>>,
    #[serde(skip_deserializing)]
    pub ban_manager: OnceLock<BanManager>,
    pub store_config: RateLimiterConfig,
    #[serde(default = "forbidden_error_response")]
    pub error_response: ErrorResponse,
}

fn forbidden_error_response() -> ErrorResponse {
    ErrorResponse::forbidden()
}

#[async_trait]
impl MiddlewareLayer for IntrusionProtection {
    async fn initialize(&self) -> Result<()> {
        // Initialize regex matchers for URL patterns
        if !self.banned_url_patterns.is_empty() {
            match RegexSet::new(&self.banned_url_patterns) {
                Ok(regex_set) => {
                    let _ = self.banned_url_pattern_matcher.set(regex_set);
                }
                Err(e) => {
                    error!("Failed to compile URL regex patterns: {:?}", e);
                }
            }
        }

        // Initialize regex matchers for header patterns
        if !self.banned_header_patterns.is_empty() {
            let mut header_matchers = HashMap::new();
            for (header_name, patterns) in &self.banned_header_patterns {
                if !patterns.is_empty() {
                    match RegexSet::new(patterns) {
                        Ok(regex_set) => {
                            header_matchers.insert(header_name.clone(), regex_set);
                        }
                        Err(e) => {
                            error!(
                                "Failed to compile header regex patterns for {}: {:?}",
                                header_name, e
                            );
                        }
                    }
                }
            }
            let _ = self.banned_header_pattern_matchers.set(header_matchers);
        }

        // Initialize rate limiter (used for tracking bans)
        // This will automatically fall back to in-memory if Redis fails
        if let Ok(limiter) = get_rate_limiter(&self.store_config).await {
            let _ = self.rate_limiter.set(limiter);
        }

        // Initialize ban manager
        // This will automatically fall back to in-memory if Redis fails
        if let Ok(manager) = get_ban_manager(&self.store_config).await {
            let _ = self.ban_manager.set(manager);
        }

        Ok(())
    }

    async fn before(
        &self,
        req: HttpRequest,
        context: &mut HttpRequestContext,
    ) -> Result<Either<HttpRequest, HttpResponse>> {
        // Get client IP address from context's service
        let client_ip = &context.addr;

        // Check if the IP is already banned
        if let Some(ban_manager) = self.ban_manager.get() {
            match ban_manager.is_banned(client_ip).await {
                Ok(Some(reason)) => {
                    info!("Request from banned IP {}: {}", client_ip, reason);
                    return Ok(Either::Right(
                        self.error_response
                            .to_http_response(req.accept().into())
                            .await,
                    ));
                }
                Err(e) => {
                    error!("Error checking IP ban status: {:?}", e);
                    // Continue processing - fail open
                }
                _ => {
                    // Not banned, continue with intrusion checks
                }
            }
        } else {
            warn!("No ban manager available for intrusion protection");
        }

        // Check for banned URL patterns
        if let Some(url_matcher) = self.banned_url_pattern_matcher.get() {
            let path = req.uri().path_and_query().map(|p| p.as_str()).unwrap_or("");

            if url_matcher.is_match(path) {
                info!("Intrusion detected: URL pattern match for {}", path);

                // Ban the IP address if possible
                if let Some(ban_manager) = self.ban_manager.get() {
                    match ban_manager
                        .ban_ip(
                            client_ip,
                            &format!("Banned URL pattern detected: {}", path),
                            Duration::from_secs(self.banned_time_seconds),
                        )
                        .await
                    {
                        Ok(_) => info!(
                            "Successfully banned IP {} for {} seconds",
                            client_ip, self.banned_time_seconds
                        ),
                        Err(e) => error!("Failed to ban IP {}: {:?}", client_ip, e),
                    }
                }

                // Always return the error response even if banning failed
                return Ok(Either::Right(
                    self.error_response
                        .to_http_response(req.accept().into())
                        .await,
                ));
            }
        }

        // Check for banned header patterns
        if let Some(header_matchers) = self.banned_header_pattern_matchers.get() {
            for (header_name, pattern_set) in header_matchers {
                if let Some(header_value) = req.header(header_name) {
                    if pattern_set.is_match(header_value) {
                        info!(
                            "Intrusion detected: Header pattern match for {} in header {}",
                            header_value, header_name
                        );

                        // Ban the IP address if possible
                        if let Some(ban_manager) = self.ban_manager.get() {
                            match ban_manager
                                .ban_ip(
                                    client_ip,
                                    &format!(
                                        "Banned header pattern detected: {} in {}",
                                        header_value, header_name
                                    ),
                                    Duration::from_secs(self.banned_time_seconds),
                                )
                                .await
                            {
                                Ok(_) => info!(
                                    "Successfully banned IP {} for {} seconds",
                                    client_ip, self.banned_time_seconds
                                ),
                                Err(e) => error!("Failed to ban IP {}: {:?}", client_ip, e),
                            }
                        }

                        // Always return the error response even if banning failed
                        return Ok(Either::Right(
                            self.error_response
                                .to_http_response(req.accept().into())
                                .await,
                        ));
                    }
                }
            }
        }

        // No intrusion detected
        Ok(Either::Left(req))
    }
}

impl FromValue for IntrusionProtection {}
