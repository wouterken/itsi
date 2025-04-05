use crate::{
    server::http_message_types::{HttpRequest, HttpResponse},
    services::itsi_http_service::HttpRequestContext,
};

use super::{FromValue, MiddlewareLayer};
use async_trait::async_trait;
use base64::{engine::general_purpose, Engine as _};
use bytes::{Bytes, BytesMut};
use either::Either;
use futures::TryStreamExt;
use http::{header, HeaderValue, Response, StatusCode};
use http_body_util::{combinators::BoxBody, BodyExt, Empty, Full};
use hyper::body::Body;
use magnus::error::Result;
use serde::Deserialize;
use sha2::{Digest, Sha256};

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
    #[serde(default = "default_true")]
    pub handle_if_none_match: bool,
}

fn default_true() -> bool {
    true
}

#[async_trait]
impl MiddlewareLayer for ETag {
    async fn before(
        &self,
        req: HttpRequest,
        context: &mut HttpRequestContext,
    ) -> Result<Either<HttpRequest, HttpResponse>> {
        // Store if-none-match header in context if present for later use in after hook
        if self.handle_if_none_match {
            if let Some(if_none_match) = req.headers().get(header::IF_NONE_MATCH) {
                if let Ok(etag_value) = if_none_match.to_str() {
                    context.set_if_none_match(Some(etag_value.to_string()));
                }
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
            _ => return resp,
        }

        // Skip if already has an ETag
        if resp.headers().contains_key(header::ETAG) {
            return resp;
        }

        // Skip if Cache-Control: no-store is present
        if let Some(cache_control) = resp.headers().get(header::CACHE_CONTROL) {
            if let Ok(cache_control_str) = cache_control.to_str() {
                if cache_control_str.contains("no-store") {
                    return resp;
                }
            }
        }

        // Check if body is a stream or fixed size using size_hint (similar to compression.rs)
        let body_size = resp.size_hint().exact();

        // Skip streaming bodies
        if body_size.is_none() {
            return resp;
        }

        // Skip if body is too small
        if body_size.unwrap_or(0) < self.min_body_size as u64 {
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
                Err(_) => return Response::from_parts(parts, BoxBody::new(Empty::new())),
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

            if let Ok(value) = HeaderValue::from_str(&formatted_etag) {
                parts.headers.insert(header::ETAG, value);
            }

            body = Full::new(full_bytes).boxed();
            formatted_etag
        };

        // Handle 304 Not Modified if we have an If-None-Match header and it matches
        if self.handle_if_none_match {
            if let Some(if_none_match) = context.get_if_none_match() {
                if if_none_match == etag_value || if_none_match == "*" {
                    // Return 304 Not Modified without the body
                    let mut not_modified = Response::new(BoxBody::new(Empty::new()));
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
        }

        // Recreate response with the original body and the ETag header
        Response::from_parts(parts, body)
    }
}

impl Default for ETag {
    fn default() -> Self {
        Self {
            r#type: ETagType::Strong,
            algorithm: HashAlgorithm::Sha256,
            min_body_size: 0,
            handle_if_none_match: true,
        }
    }
}

impl FromValue for ETag {}
