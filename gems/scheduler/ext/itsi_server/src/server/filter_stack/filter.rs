use crate::server::{
    itsi_service::ItsiService,
    types::{HttpRequest, HttpResponse},
};

use super::filters::*;
use async_trait::async_trait;
use either::Either;
use magnus::error::Result;

#[derive(Debug)]
pub enum Filter {
    AuthBasic(AuthBasic),
    AuthJwt(Box<AuthJwt>),
    AuthAPIKey(AuthAPIKey),
    Endpoint(Endpoint),
    RateLimit(RateLimit),
    Cors(Box<Cors>),
    StaticAssets(StaticAssets),
    Compression(Compression),
    Logging(Logging),
    RackApp(RackApp),
}
impl Filter {
    pub(crate) fn preload(&self) -> Result<()> {
        if let Filter::RackApp(filter) = self {
            filter.preload()?;
        }
        Ok(())
    }
}

#[async_trait]
impl FilterLayer for Filter {
    async fn before(
        &self,
        req: HttpRequest,
        context: &ItsiService,
    ) -> Result<Either<HttpRequest, HttpResponse>> {
        match self {
            Filter::AuthBasic(filter) => filter.before(req, context).await,
            Filter::AuthJwt(filter) => filter.before(req, context).await,
            Filter::AuthAPIKey(filter) => filter.before(req, context).await,
            Filter::Endpoint(filter) => filter.before(req, context).await,
            Filter::RateLimit(filter) => filter.before(req, context).await,
            Filter::Cors(filter) => filter.before(req, context).await,
            Filter::StaticAssets(filter) => filter.before(req, context).await,
            Filter::Compression(filter) => filter.before(req, context).await,
            Filter::Logging(filter) => filter.before(req, context).await,
            Filter::RackApp(filter) => filter.before(req, context).await,
        }
    }

    async fn after(&self, res: HttpResponse) -> HttpResponse {
        match self {
            Filter::AuthBasic(filter) => filter.after(res).await,
            Filter::AuthJwt(filter) => filter.after(res).await,
            Filter::AuthAPIKey(filter) => filter.after(res).await,
            Filter::Endpoint(filter) => filter.after(res).await,
            Filter::RateLimit(filter) => filter.after(res).await,
            Filter::Cors(filter) => filter.after(res).await,
            Filter::StaticAssets(filter) => filter.after(res).await,
            Filter::Compression(filter) => filter.after(res).await,
            Filter::Logging(filter) => filter.after(res).await,
            Filter::RackApp(filter) => filter.after(res).await,
        }
    }
}
