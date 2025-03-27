use async_trait::async_trait;
use either::Either;
use itsi_tracing::*;
use magnus::error::Result;
use serde::Deserialize;

use crate::server::itsi_service::RequestContext;
use crate::server::types::{HttpRequest, HttpResponse};

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

impl LogMiddlewareLevel {
    pub fn log(&self, message: String) {
        match self {
            LogMiddlewareLevel::Trace => trace!(message),
            LogMiddlewareLevel::Debug => debug!(message),
            LogMiddlewareLevel::Info => info!(message),
            LogMiddlewareLevel::Warn => warn!(message),
            LogMiddlewareLevel::Error => error!(message),
        }
    }
}

#[async_trait]
impl MiddlewareLayer for LogRequests {
    async fn initialize(&self) -> Result<()> {
        Ok(())
    }

    async fn before(
        &self,
        req: HttpRequest,
        context: &mut RequestContext,
    ) -> Result<Either<HttpRequest, HttpResponse>> {
        context.track_start_time();
        if let Some(LogConfig { level, format }) = self.before.as_ref() {
            level.log(format.rewrite_request(&req, context));
        }

        Ok(Either::Left(req))
    }

    async fn after(&self, resp: HttpResponse, context: &mut RequestContext) -> HttpResponse {
        if let Some(LogConfig { level, format }) = self.after.as_ref() {
            level.log(format.rewrite_response(&resp, context));
        }

        resp
    }
}

impl FromValue for LogRequests {}
