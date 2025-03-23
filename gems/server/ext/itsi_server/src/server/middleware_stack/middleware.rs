use super::middlewares::*;
use crate::server::{
    itsi_service::RequestContext,
    types::{HttpRequest, HttpResponse},
};
use async_trait::async_trait;
use either::Either;
use magnus::error::Result;
use std::cmp::Ordering;

#[derive(Debug)]
pub enum Middleware {
    AuthBasic(AuthBasic),
    AuthJwt(Box<AuthJwt>),
    AuthAPIKey(AuthAPIKey),
    RateLimit(RateLimit),
    Cors(Box<Cors>),
    StaticAssets(StaticAssets),
    Compression(Compression),
    Logging(Logging),
    Redirect(Redirect),
    Proxy(Proxy),
    RubyApp(RubyApp),
}

#[async_trait]
impl MiddlewareLayer for Middleware {
    /// Called just once, to initialize the middleware state.
    async fn initialize(&self) -> Result<()> {
        match self {
            Middleware::AuthBasic(filter) => filter.initialize().await,
            Middleware::AuthJwt(filter) => filter.initialize().await,
            Middleware::AuthAPIKey(filter) => filter.initialize().await,
            Middleware::RateLimit(filter) => filter.initialize().await,
            Middleware::Cors(filter) => filter.initialize().await,
            Middleware::StaticAssets(filter) => filter.initialize().await,
            Middleware::Compression(filter) => filter.initialize().await,
            Middleware::Logging(filter) => filter.initialize().await,
            Middleware::Redirect(filter) => filter.initialize().await,
            Middleware::Proxy(filter) => filter.initialize().await,
            Middleware::RubyApp(filter) => filter.initialize().await,
        }
    }

    async fn before(
        &self,
        req: HttpRequest,
        context: &mut RequestContext,
    ) -> Result<Either<HttpRequest, HttpResponse>> {
        match self {
            Middleware::AuthBasic(filter) => filter.before(req, context).await,
            Middleware::AuthJwt(filter) => filter.before(req, context).await,
            Middleware::AuthAPIKey(filter) => filter.before(req, context).await,
            Middleware::RateLimit(filter) => filter.before(req, context).await,
            Middleware::Cors(filter) => filter.before(req, context).await,
            Middleware::StaticAssets(filter) => filter.before(req, context).await,
            Middleware::Compression(filter) => filter.before(req, context).await,
            Middleware::Logging(filter) => filter.before(req, context).await,
            Middleware::Redirect(filter) => filter.before(req, context).await,
            Middleware::Proxy(filter) => filter.before(req, context).await,
            Middleware::RubyApp(filter) => filter.before(req, context).await,
        }
    }

    async fn after(&self, res: HttpResponse, context: &mut RequestContext) -> HttpResponse {
        match self {
            Middleware::AuthBasic(filter) => filter.after(res, context).await,
            Middleware::AuthJwt(filter) => filter.after(res, context).await,
            Middleware::AuthAPIKey(filter) => filter.after(res, context).await,
            Middleware::RateLimit(filter) => filter.after(res, context).await,
            Middleware::Cors(filter) => filter.after(res, context).await,
            Middleware::StaticAssets(filter) => filter.after(res, context).await,
            Middleware::Compression(filter) => filter.after(res, context).await,
            Middleware::Logging(filter) => filter.after(res, context).await,
            Middleware::Redirect(filter) => filter.after(res, context).await,
            Middleware::Proxy(filter) => filter.after(res, context).await,
            Middleware::RubyApp(filter) => filter.after(res, context).await,
        }
    }
}

impl Middleware {
    fn variant_order(&self) -> usize {
        match self {
            Middleware::Redirect(_) => 0,
            Middleware::Logging(_) => 1,
            Middleware::AuthBasic(_) => 2,
            Middleware::AuthJwt(_) => 3,
            Middleware::AuthAPIKey(_) => 4,
            Middleware::RateLimit(_) => 5,
            Middleware::Compression(_) => 6,
            Middleware::Proxy(_) => 7,
            Middleware::Cors(_) => 8,
            Middleware::StaticAssets(_) => 9,
            Middleware::RubyApp(_) => 10,
        }
    }
}

impl PartialEq for Middleware {
    fn eq(&self, other: &Self) -> bool {
        self.variant_order() == other.variant_order()
    }
}

impl Eq for Middleware {}

impl PartialOrd for Middleware {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.variant_order().cmp(&other.variant_order()))
    }
}

impl Ord for Middleware {
    fn cmp(&self, other: &Self) -> Ordering {
        self.variant_order().cmp(&other.variant_order())
    }
}
