use std::sync::OnceLock;

use super::{FromValue, MiddlewareLayer};
use crate::server::http_message_types::{HttpRequest, HttpResponse};
use crate::services::itsi_http_service::HttpRequestContext;
use async_trait::async_trait;
use bytes::Bytes;
use derive_more::Debug;
use either::Either;
use http::{HeaderMap, HeaderName, HeaderValue, Response, StatusCode};
use http_body_util::combinators::BoxBody;
use http_body_util::Full;
use itsi_error::ItsiError;
use magnus::error::Result;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct StaticResponse {
    code: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    #[serde(skip)]
    header_map: OnceLock<HeaderMap>,
    #[serde(skip)]
    body_bytes: OnceLock<Full<Bytes>>,
    #[serde(skip)]
    status_code: OnceLock<StatusCode>,
}

#[async_trait]
impl MiddlewareLayer for StaticResponse {
    async fn initialize(&self) -> Result<()> {
        let mut header_map = HeaderMap::new();
        for (key, value) in self.headers.iter() {
            if let (Ok(hn), Ok(hv)) = (key.parse::<HeaderName>(), value.parse::<HeaderValue>()) {
                header_map.insert(hn, hv);
            }
        }
        self.header_map
            .set(header_map)
            .map_err(|_| ItsiError::new("Failed to set headers"))?;
        self.body_bytes
            .set(Full::new(Bytes::from(self.body.clone())))
            .map_err(|_| ItsiError::new("Failed to set body bytes"))?;
        self.status_code
            .set(StatusCode::from_u16(self.code).unwrap_or(StatusCode::OK))
            .map_err(|_| ItsiError::new("Failed to set status code"))?;
        Ok(())
    }

    async fn before(
        &self,
        _req: HttpRequest,
        _context: &mut HttpRequestContext,
    ) -> Result<Either<HttpRequest, HttpResponse>> {
        let mut resp = Response::new(BoxBody::new(self.body_bytes.get().unwrap().clone()));
        *resp.status_mut() = *self.status_code.get().unwrap();
        *resp.headers_mut() = self.header_map.get().unwrap().clone();

        Ok(Either::Right(resp))
    }
}

impl FromValue for StaticResponse {}
