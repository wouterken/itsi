use crate::{
    server::http_message_types::{HttpBody, HttpRequest, HttpResponse},
    services::itsi_http_service::HttpRequestContext,
};

use super::{FromValue, MiddlewareLayer};
use async_trait::async_trait;
use base64::{engine::general_purpose, Engine as _};
use bytes::{Bytes, BytesMut};
use either::Either;
use futures::TryStreamExt;
use http::{header, HeaderValue, Response, StatusCode};
use http_body_util::BodyExt;
use hyper::body::Body;
use magnus::error::Result;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tracing::debug;

#[derive(Debug, Clone, Copy, Deserialize, Default)]
pub enum ETagType {
    #[serde(rename = "strong")]
    #[default]
    Strong,
    #[serde(rename = "weak")]
    Weak,
}

#[derive(Debug, Clone, Copy, Deserialize, Default)]
pub enum HashAlgorithm {
    #[serde(rename = "sha256")]
    #[default]
    Sha256,
    #[serde(rename = "md5")]
    Md5,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ETag {
    #[serde(default)]
    pub r#type: ETagType,
    #[serde(default)]
    pub algorithm: HashAlgorithm,
    #[serde(default)]
    pub min_body_size: usize,
}

#[async_trait]
impl MiddlewareLayer for ETag {
    async fn before(
        &self,
        req: HttpRequest,
        context: &mut HttpRequestContext,
    ) -> Result<Either<HttpRequest, HttpResponse>> {
        // Store if-none-match header in context if present for later use in after hook
        if let Some(if_none_match) = req.headers().get(header::IF_NONE_MATCH) {
            debug!(target: "middleware::etag", "Received If-None-Match header: {:?}", if_none_match);
            if let Ok(etag_value) = if_none_match.to_str() {
                context.set_if_none_match(Some(etag_value.to_string()));
            }
        }

        Ok(Either::Left(req))
    }

    async fn after(&self, resp: HttpResponse, context: &mut HttpRequestContext) -> HttpResponse {
        // Skip for error responses or responses that shouldn't have ETags
        match resp.status() {
            StatusCode::OK
            | StatusCode::CREATED
            | StatusCode::ACCEPTED
            | StatusCode::NON_AUTHORITATIVE_INFORMATION
            | StatusCode::NO_CONTENT
            | StatusCode::PARTIAL_CONTENT => {}
            _ => {
                debug!(target: "middleware::etag", "Skipping ETag middleware for ineligible response");
                return resp;
            }
        }

        if let Some(cache_control) = resp.headers().get(header::CACHE_CONTROL) {
            if let Ok(cache_control_str) = cache_control.to_str() {
                if cache_control_str.contains("no-store") {
                    debug!(target: "middleware::etag", "Skipping ETag for no-store response");
                    return resp;
                }
            }
        }

        let body_size = resp.size_hint().exact();

        if body_size.is_none() {
            debug!(target: "middleware::etag", "Skipping ETag for streaming response");
            return resp;
        }

        if body_size.unwrap_or(0) < self.min_body_size as u64 {
            debug!(target: "middleware::etag", "Skipping ETag for small response");
            return resp;
        }

        let (mut parts, mut body) = resp.into_parts();
        let etag_value = if let Some(existing_etag) = parts.headers.get(header::ETAG) {
            existing_etag.to_str().unwrap_or("").to_string()
        } else {
            // Get the full bytes from the body
            let full_bytes: Bytes = match body
                .into_data_stream()
                .try_fold(BytesMut::new(), |mut acc, chunk| async move {
                    acc.extend_from_slice(&chunk);
                    Ok(acc)
                })
                .await
            {
                Ok(bytes_mut) => bytes_mut.freeze(),
                Err(_) => return Response::from_parts(parts, HttpBody::empty()),
            };

            let computed_etag = match self.algorithm {
                HashAlgorithm::Sha256 => {
                    let mut hasher = Sha256::new();
                    hasher.update(&full_bytes);
                    let result = hasher.finalize();
                    general_purpose::STANDARD.encode(result)
                }
                HashAlgorithm::Md5 => {
                    let digest = md5::compute(&full_bytes);
                    format!("{:x}", digest)
                }
            };

            let formatted_etag = match self.r#type {
                ETagType::Strong => format!("\"{}\"", computed_etag),
                ETagType::Weak => format!("W/\"{}\"", computed_etag),
            };

            debug!(target: "middleware::etag", "Computed ETag for response {}", formatted_etag);
            if let Ok(value) = HeaderValue::from_str(&formatted_etag) {
                parts.headers.insert(header::ETAG, value);
            }

            body = HttpBody::full(full_bytes);
            formatted_etag
        };

        if let Some(if_none_match) = context.get_if_none_match() {
            if if_none_match == etag_value || if_none_match == "*" {
                // Return 304 Not Modified without the body
                let mut not_modified = Response::new(HttpBody::empty());
                *not_modified.status_mut() = StatusCode::NOT_MODIFIED;
                // Copy headers we want to preserve
                for (name, value) in parts.headers.iter() {
                    if matches!(
                        name,
                        &header::CACHE_CONTROL
                            | &header::CONTENT_LOCATION
                            | &header::DATE
                            | &header::ETAG
                            | &header::EXPIRES
                            | &header::VARY
                    ) {
                        not_modified.headers_mut().insert(name, value.clone());
                    }
                }
                return not_modified;
            }
        }

        Response::from_parts(parts, body)
    }
}

impl Default for ETag {
    fn default() -> Self {
        Self {
            r#type: ETagType::Strong,
            algorithm: HashAlgorithm::Sha256,
            min_body_size: 0,
        }
    }
}

impl FromValue for ETag {}
