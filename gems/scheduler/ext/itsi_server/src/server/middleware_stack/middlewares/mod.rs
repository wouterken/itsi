mod auth_api_key;
mod auth_basic;
mod auth_jwt;
mod compression;
mod cors;
mod endpoint;
mod error_response;
mod header_interpretation;
mod logging;
mod proxy;
mod rate_limit;
mod redirect;
mod ruby_app;
mod static_assets;
mod string_rewrite;
mod token_source;
use async_trait::async_trait;
use either::Either;
use magnus::error::Result;
use magnus::Value;
use serde::Deserialize;
use serde_magnus::deserialize;

pub use auth_api_key::AuthAPIKey;
pub use auth_basic::AuthBasic;
pub use auth_jwt::AuthJwt;
pub use compression::Compression;
pub use compression::CompressionAlgorithm;
pub use cors::Cors;
pub use endpoint::Endpoint;
pub use logging::Logging;
pub use proxy::Proxy;
pub use rate_limit::RateLimit;
pub use redirect::Redirect;
pub use ruby_app::RubyApp;
pub use static_assets::StaticAssets;

use crate::server::itsi_service::RequestContext;
use crate::server::types::{HttpRequest, HttpResponse};

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
    /// The “before” hook. By default, it passes through the request.
    async fn before(
        &self,
        req: HttpRequest,
        _context: &mut RequestContext,
    ) -> Result<Either<HttpRequest, HttpResponse>> {
        Ok(Either::Left(req))
    }

    /// The “after” hook. By default, it passes through the response.
    async fn after(&self, resp: HttpResponse, context: &mut RequestContext) -> HttpResponse {
        resp
    }
}
