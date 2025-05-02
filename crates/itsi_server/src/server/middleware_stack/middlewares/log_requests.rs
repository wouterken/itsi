use async_trait::async_trait;
use either::Either;
use itsi_tracing::*;
use magnus::error::Result;
use serde::Deserialize;
use tracing::enabled;

use crate::server::http_message_types::{HttpRequest, HttpResponse};
use crate::services::itsi_http_service::HttpRequestContext;

use super::string_rewrite::StringRewrite;
use super::{FromValue, MiddlewareLayer};

/// Logging middleware for HTTP requests and responses
///
/// Supports customizable log formats with placeholders
#[derive(Debug, Clone, Deserialize)]
pub struct LogRequests {
    pub before: Option<LogConfig>,
    pub after: Option<LogConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LogConfig {
    level: LogMiddlewareLevel,
    format: StringRewrite,
}

#[derive(Debug, Clone, Deserialize)]
pub enum LogMiddlewareLevel {
    #[serde(rename(deserialize = "INFO"))]
    Info,
    #[serde(rename(deserialize = "TRACE"))]
    Trace,
    #[serde(rename(deserialize = "DEBUG"))]
    Debug,
    #[serde(rename(deserialize = "WARN"))]
    Warn,
    #[serde(rename(deserialize = "ERROR"))]
    Error,
}

#[async_trait]
impl MiddlewareLayer for LogRequests {
    async fn initialize(&self) -> Result<()> {
        Ok(())
    }

    async fn before(
        &self,
        req: HttpRequest,
        context: &mut HttpRequestContext,
    ) -> Result<Either<HttpRequest, HttpResponse>> {
        context.init_logging_params();
        if let Some(LogConfig { level, format }) = self.before.as_ref() {
            match level {
                LogMiddlewareLevel::Trace => {
                    if enabled!(target: "middleware::log_requests", tracing::Level::TRACE) {
                        let message = format.rewrite_request(&req, context);
                        trace!(target: "middleware::log_requests", message);
                    }
                }
                LogMiddlewareLevel::Debug => {
                    if enabled!(target: "middleware::log_requests", tracing::Level::DEBUG) {
                        let message = format.rewrite_request(&req, context);
                        debug!(target: "middleware::log_requests", message);
                    }
                }
                LogMiddlewareLevel::Info => {
                    if enabled!(target: "middleware::log_requests", tracing::Level::INFO) {
                        let message = format.rewrite_request(&req, context);
                        info!(target: "middleware::log_requests", message);
                    }
                }
                LogMiddlewareLevel::Warn => {
                    if enabled!(target: "middleware::log_requests", tracing::Level::WARN) {
                        let message = format.rewrite_request(&req, context);
                        warn!(target: "middleware::log_requests", message);
                    }
                }
                LogMiddlewareLevel::Error => {
                    if enabled!(target: "middleware::log_requests", tracing::Level::ERROR) {
                        let message = format.rewrite_request(&req, context);
                        error!(target: "middleware::log_requests", message);
                    }
                }
            }
        }

        Ok(Either::Left(req))
    }

    async fn after(&self, resp: HttpResponse, context: &mut HttpRequestContext) -> HttpResponse {
        if let Some(LogConfig { level, format }) = self.after.as_ref() {
            match level {
                LogMiddlewareLevel::Trace => {
                    if enabled!(target: "middleware::log_requests", tracing::Level::TRACE) {
                        let message = format.rewrite_response(&resp, context);
                        trace!(target: "middleware::log_requests", message);
                    }
                }
                LogMiddlewareLevel::Debug => {
                    if enabled!(target: "middleware::log_requests", tracing::Level::DEBUG) {
                        let message = format.rewrite_response(&resp, context);
                        debug!(target: "middleware::log_requests", message);
                    }
                }
                LogMiddlewareLevel::Info => {
                    if enabled!(target: "middleware::log_requests", tracing::Level::INFO) {
                        let message = format.rewrite_response(&resp, context);
                        info!(target: "middleware::log_requests", message);
                    }
                }
                LogMiddlewareLevel::Warn => {
                    if enabled!(target: "middleware::log_requests", tracing::Level::WARN) {
                        let message = format.rewrite_response(&resp, context);
                        warn!(target: "middleware::log_requests", message);
                    }
                }
                LogMiddlewareLevel::Error => {
                    if enabled!(target: "middleware::log_requests", tracing::Level::ERROR) {
                        let message = format.rewrite_response(&resp, context);
                        error!(target: "middleware::log_requests", message);
                    }
                }
            }
        }

        resp
    }
}

impl FromValue for LogRequests {}
