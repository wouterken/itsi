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
    DenyList(DenyList),
    AllowList(AllowList),
    AuthBasic(AuthBasic),
    AuthJwt(Box<AuthJwt>),
    AuthAPIKey(AuthAPIKey),
    RateLimit(RateLimit),
    RequestHeaders(RequestHeaders),
    ResponseHeaders(ResponseHeaders),
    Cors(Box<Cors>),
    StaticAssets(StaticAssets),
    Compression(Compression),
    LogRequests(LogRequests),
    Redirect(Redirect),
    Proxy(Proxy),
    RubyApp(RubyApp),
}

#[async_trait]
impl MiddlewareLayer for Middleware {
    /// Called just once, to initialize the middleware state.
    async fn initialize(&self) -> Result<()> {
        match self {
            Middleware::DenyList(filter) => filter.initialize().await,
            Middleware::AllowList(filter) => filter.initialize().await,
            Middleware::AuthBasic(filter) => filter.initialize().await,
            Middleware::AuthJwt(filter) => filter.initialize().await,
            Middleware::AuthAPIKey(filter) => filter.initialize().await,
            Middleware::RateLimit(filter) => filter.initialize().await,
            Middleware::RequestHeaders(filter) => filter.initialize().await,
            Middleware::ResponseHeaders(filter) => filter.initialize().await,
            Middleware::Cors(filter) => filter.initialize().await,
            Middleware::StaticAssets(filter) => filter.initialize().await,
            Middleware::Compression(filter) => filter.initialize().await,
            Middleware::LogRequests(filter) => filter.initialize().await,
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
            Middleware::DenyList(filter) => filter.before(req, context).await,
            Middleware::AllowList(filter) => filter.before(req, context).await,
            Middleware::AuthBasic(filter) => filter.before(req, context).await,
            Middleware::AuthJwt(filter) => filter.before(req, context).await,
            Middleware::AuthAPIKey(filter) => filter.before(req, context).await,
            Middleware::RequestHeaders(filter) => filter.before(req, context).await,
            Middleware::ResponseHeaders(filter) => filter.before(req, context).await,
            Middleware::RateLimit(filter) => filter.before(req, context).await,
            Middleware::Cors(filter) => filter.before(req, context).await,
            Middleware::StaticAssets(filter) => filter.before(req, context).await,
            Middleware::Compression(filter) => filter.before(req, context).await,
            Middleware::LogRequests(filter) => filter.before(req, context).await,
            Middleware::Redirect(filter) => filter.before(req, context).await,
            Middleware::Proxy(filter) => filter.before(req, context).await,
            Middleware::RubyApp(filter) => filter.before(req, context).await,
        }
    }

    async fn after(&self, res: HttpResponse, context: &mut RequestContext) -> HttpResponse {
        match self {
            Middleware::DenyList(filter) => filter.after(res, context).await,
            Middleware::AllowList(filter) => filter.after(res, context).await,
            Middleware::AuthBasic(filter) => filter.after(res, context).await,
            Middleware::AuthJwt(filter) => filter.after(res, context).await,
            Middleware::AuthAPIKey(filter) => filter.after(res, context).await,
            Middleware::RateLimit(filter) => filter.after(res, context).await,
            Middleware::RequestHeaders(filter) => filter.after(res, context).await,
            Middleware::ResponseHeaders(filter) => filter.after(res, context).await,
            Middleware::Cors(filter) => filter.after(res, context).await,
            Middleware::StaticAssets(filter) => filter.after(res, context).await,
            Middleware::Compression(filter) => filter.after(res, context).await,
            Middleware::LogRequests(filter) => filter.after(res, context).await,
            Middleware::Redirect(filter) => filter.after(res, context).await,
            Middleware::Proxy(filter) => filter.after(res, context).await,
            Middleware::RubyApp(filter) => filter.after(res, context).await,
        }
    }
}

impl Middleware {
    fn variant_order(&self) -> usize {
        match self {
            Middleware::DenyList(_) => 0,
            Middleware::AllowList(_) => 1,
            Middleware::Redirect(_) => 2,
            Middleware::LogRequests(_) => 3,
            Middleware::RequestHeaders(_) => 4,
            Middleware::ResponseHeaders(_) => 5,
            Middleware::AuthBasic(_) => 6,
            Middleware::AuthJwt(_) => 7,
            Middleware::AuthAPIKey(_) => 8,
            Middleware::RateLimit(_) => 9,
            Middleware::Compression(_) => 10,
            Middleware::Proxy(_) => 11,
            Middleware::Cors(_) => 12,
            Middleware::StaticAssets(_) => 13,
            Middleware::RubyApp(_) => 14,
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
