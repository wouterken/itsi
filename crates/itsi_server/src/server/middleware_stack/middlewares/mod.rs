mod allow_list;
mod auth_api_key;
mod auth_basic;
mod auth_jwt;
mod cache_control;
mod compression;
mod cors;
mod deny_list;
mod error_response;
mod etag;
mod header_interpretation;
mod intrusion_protection;
mod log_requests;
mod max_body;
mod proxy;
mod rate_limit;
mod redirect;
mod request_headers;
mod response_headers;
mod ruby_app;
mod static_assets;
mod string_rewrite;
mod token_source;

pub use allow_list::AllowList;
use async_trait::async_trait;
pub use auth_api_key::AuthAPIKey;
pub use auth_basic::AuthBasic;
pub use auth_jwt::AuthJwt;
pub use cache_control::CacheControl;
pub use compression::Compression;
pub use compression::CompressionAlgorithm;
pub use cors::Cors;
pub use deny_list::DenyList;
use either::Either;
pub use error_response::ErrorResponse;
pub use etag::ETag;
pub use intrusion_protection::IntrusionProtection;
pub use log_requests::LogRequests;
use magnus::error::Result;
use magnus::Value;
pub use max_body::MaxBody;
pub use proxy::Proxy;
pub use rate_limit::RateLimit;
pub use redirect::Redirect;
pub use request_headers::RequestHeaders;
pub use response_headers::ResponseHeaders;
pub use ruby_app::RubyApp;
use serde::Deserialize;
use serde_magnus::deserialize;
pub use static_assets::StaticAssets;

use crate::server::http_message_types::HttpRequest;
use crate::server::http_message_types::HttpResponse;
use crate::services::itsi_http_service::HttpRequestContext;

pub trait FromValue: Sized + Send + Sync + 'static {
    fn from_value(value: Value) -> Result<Self>
    where
        Self: Deserialize<'static>,
    {
        deserialize(value)
    }
}

#[async_trait]
pub trait MiddlewareLayer: Sized + Send + Sync + 'static {
    /// Called just once, to initialize the middleware state.
    async fn initialize(&self) -> Result<()> {
        Ok(())
    }
    /// The "before" hook. By default, it passes through the request.
    async fn before(
        &self,
        req: HttpRequest,
        _context: &mut HttpRequestContext,
    ) -> Result<Either<HttpRequest, HttpResponse>> {
        Ok(Either::Left(req))
    }

    /// The "after" hook. By default, it passes through the response.
    async fn after(&self, resp: HttpResponse, _context: &mut HttpRequestContext) -> HttpResponse {
        resp
    }
}
