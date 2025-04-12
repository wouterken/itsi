use crate::{
    server::http_message_types::{HttpRequest, HttpResponse},
    services::itsi_http_service::HttpRequestContext,
};

use super::middlewares::*;

use async_trait::async_trait;
use either::Either;
use magnus::error::Result;
use std::{cmp::Ordering, sync::Arc};

#[derive(Debug, Clone)]
pub enum Middleware {
    AllowList(Arc<AllowList>),
    AuthAPIKey(Arc<AuthAPIKey>),
    AuthBasic(Arc<AuthBasic>),
    AuthJwt(Arc<AuthJwt>),
    CacheControl(Arc<CacheControl>),
    Compression(Arc<Compression>),
    Cors(Arc<Cors>),
    Csp(Arc<Csp>),
    DenyList(Arc<DenyList>),
    ETag(Arc<ETag>),
    IntrusionProtection(Arc<IntrusionProtection>),
    LogRequests(Arc<LogRequests>),
    MaxBody(Arc<MaxBody>),
    Proxy(Arc<Proxy>),
    RateLimit(Arc<RateLimit>),
    Redirect(Arc<Redirect>),
    RequestHeaders(Arc<RequestHeaders>),
    ResponseHeaders(Arc<ResponseHeaders>),
    RubyApp(Arc<RubyApp>),
    StaticAssets(Arc<StaticAssets>),
    StaticResponse(Arc<StaticResponse>),
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
            Middleware::IntrusionProtection(filter) => filter.initialize().await,
            Middleware::MaxBody(filter) => filter.initialize().await,
            Middleware::RateLimit(filter) => filter.initialize().await,
            Middleware::RequestHeaders(filter) => filter.initialize().await,
            Middleware::ResponseHeaders(filter) => filter.initialize().await,
            Middleware::CacheControl(filter) => filter.initialize().await,
            Middleware::Cors(filter) => filter.initialize().await,
            Middleware::Csp(filter) => filter.initialize().await,
            Middleware::ETag(filter) => filter.initialize().await,
            Middleware::StaticAssets(filter) => filter.initialize().await,
            Middleware::StaticResponse(filter) => filter.initialize().await,
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
        context: &mut HttpRequestContext,
    ) -> Result<Either<HttpRequest, HttpResponse>> {
        match self {
            Middleware::DenyList(filter) => filter.before(req, context).await,
            Middleware::AllowList(filter) => filter.before(req, context).await,
            Middleware::AuthBasic(filter) => filter.before(req, context).await,
            Middleware::AuthJwt(filter) => filter.before(req, context).await,
            Middleware::AuthAPIKey(filter) => filter.before(req, context).await,
            Middleware::IntrusionProtection(filter) => filter.before(req, context).await,
            Middleware::MaxBody(filter) => filter.before(req, context).await,
            Middleware::RequestHeaders(filter) => filter.before(req, context).await,
            Middleware::ResponseHeaders(filter) => filter.before(req, context).await,
            Middleware::RateLimit(filter) => filter.before(req, context).await,
            Middleware::CacheControl(filter) => filter.before(req, context).await,
            Middleware::Cors(filter) => filter.before(req, context).await,
            Middleware::Csp(filter) => filter.before(req, context).await,
            Middleware::ETag(filter) => filter.before(req, context).await,
            Middleware::StaticAssets(filter) => filter.before(req, context).await,
            Middleware::StaticResponse(filter) => filter.before(req, context).await,
            Middleware::Compression(filter) => filter.before(req, context).await,
            Middleware::LogRequests(filter) => filter.before(req, context).await,
            Middleware::Redirect(filter) => filter.before(req, context).await,
            Middleware::Proxy(filter) => filter.before(req, context).await,
            Middleware::RubyApp(filter) => filter.before(req, context).await,
        }
    }

    async fn after(&self, res: HttpResponse, context: &mut HttpRequestContext) -> HttpResponse {
        match self {
            Middleware::DenyList(filter) => filter.after(res, context).await,
            Middleware::AllowList(filter) => filter.after(res, context).await,
            Middleware::AuthBasic(filter) => filter.after(res, context).await,
            Middleware::AuthJwt(filter) => filter.after(res, context).await,
            Middleware::AuthAPIKey(filter) => filter.after(res, context).await,
            Middleware::IntrusionProtection(filter) => filter.after(res, context).await,
            Middleware::MaxBody(filter) => filter.after(res, context).await,
            Middleware::RateLimit(filter) => filter.after(res, context).await,
            Middleware::RequestHeaders(filter) => filter.after(res, context).await,
            Middleware::ResponseHeaders(filter) => filter.after(res, context).await,
            Middleware::CacheControl(filter) => filter.after(res, context).await,
            Middleware::Csp(filter) => filter.after(res, context).await,
            Middleware::Cors(filter) => filter.after(res, context).await,
            Middleware::ETag(filter) => filter.after(res, context).await,
            Middleware::StaticAssets(filter) => filter.after(res, context).await,
            Middleware::StaticResponse(filter) => filter.after(res, context).await,
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
            Middleware::IntrusionProtection(_) => 2,
            Middleware::Redirect(_) => 3,
            Middleware::LogRequests(_) => 4,
            Middleware::CacheControl(_) => 5,
            Middleware::RequestHeaders(_) => 6,
            Middleware::ResponseHeaders(_) => 7,
            Middleware::MaxBody(_) => 8,
            Middleware::AuthBasic(_) => 9,
            Middleware::AuthJwt(_) => 10,
            Middleware::AuthAPIKey(_) => 11,
            Middleware::RateLimit(_) => 12,
            Middleware::ETag(_) => 13,
            Middleware::Csp(_) => 14,
            Middleware::Compression(_) => 15,
            Middleware::Proxy(_) => 16,
            Middleware::Cors(_) => 17,
            Middleware::StaticResponse(_) => 18,
            Middleware::StaticAssets(_) => 19,
            Middleware::RubyApp(_) => 20,
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
