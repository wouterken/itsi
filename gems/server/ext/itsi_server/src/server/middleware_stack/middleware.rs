use super::middlewares::*;
use crate::server::{
    itsi_service::ItsiService,
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
    Endpoint(Endpoint),
    RateLimit(RateLimit),
    Cors(Box<Cors>),
    StaticAssets(StaticAssets),
    Compression(Compression),
    Logging(Logging),
    RubyApp(RubyApp),
}

#[async_trait]
impl MiddlewareLayer for Middleware {
    async fn before(
        &self,
        req: HttpRequest,
        context: &ItsiService,
    ) -> Result<Either<HttpRequest, HttpResponse>> {
        match self {
            Middleware::AuthBasic(filter) => filter.before(req, context).await,
            Middleware::AuthJwt(filter) => filter.before(req, context).await,
            Middleware::AuthAPIKey(filter) => filter.before(req, context).await,
            Middleware::Endpoint(filter) => filter.before(req, context).await,
            Middleware::RateLimit(filter) => filter.before(req, context).await,
            Middleware::Cors(filter) => filter.before(req, context).await,
            Middleware::StaticAssets(filter) => filter.before(req, context).await,
            Middleware::Compression(filter) => filter.before(req, context).await,
            Middleware::Logging(filter) => filter.before(req, context).await,
            Middleware::RubyApp(filter) => filter.before(req, context).await,
        }
    }

    async fn after(&self, res: HttpResponse) -> HttpResponse {
        match self {
            Middleware::AuthBasic(filter) => filter.after(res).await,
            Middleware::AuthJwt(filter) => filter.after(res).await,
            Middleware::AuthAPIKey(filter) => filter.after(res).await,
            Middleware::Endpoint(filter) => filter.after(res).await,
            Middleware::RateLimit(filter) => filter.after(res).await,
            Middleware::Cors(filter) => filter.after(res).await,
            Middleware::StaticAssets(filter) => filter.after(res).await,
            Middleware::Compression(filter) => filter.after(res).await,
            Middleware::Logging(filter) => filter.after(res).await,
            Middleware::RubyApp(filter) => filter.after(res).await,
        }
    }
}

impl Middleware {
    fn variant_order(&self) -> usize {
        match self {
            Middleware::Logging(_) => 0,
            Middleware::AuthBasic(_) => 1,
            Middleware::AuthJwt(_) => 2,
            Middleware::AuthAPIKey(_) => 3,
            Middleware::RateLimit(_) => 4,
            Middleware::Cors(_) => 5,
            Middleware::Compression(_) => 6,
            Middleware::StaticAssets(_) => 7,
            Middleware::Endpoint(_) => 8,
            Middleware::RubyApp(_) => 9,
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
