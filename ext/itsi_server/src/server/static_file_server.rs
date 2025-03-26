use bytes::Bytes;
use chrono::{DateTime, Utc};
use http::{Response, StatusCode};
use http_body_util::{combinators::BoxBody, Full};
use moka::sync::Cache;
use serde::Deserialize;
use std::{
    collections::HashMap,
    convert::Infallible,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant, SystemTime},
};
use tokio::sync::Mutex;
use tokio::{fs::File, io::AsyncReadExt};
use tracing::warn;

use super::middleware_stack::ErrorResponse; 

pub async fn serve(
    &self,
    key: &str,
    serve_range: ServeRange,
    if_modified_since: Option<SystemTime>,
    is_head_request: bool,
) -> http::Response<BoxBody<Bytes, Infallible>> {
    let resolved = self.resolve(key).await;
    match resolved {
        Ok(resolved) => {
            let start = match serve_range {
                ServeRange::Full => 0,
                ServeRange::All => 0,
                ServeRange::Range(start, _end) => start,
            };
            let end = match serve_range {
                ServeRange::Full => u64::MAX,
                ServeRange::All => u64::MAX,
                ServeRange::Range(_start, end) => end,
            };
            let is_range_request = matches!(serve_range, ServeRange::Range(_, _));

            if let Some(cache_entry) = self.cache.get(&resolved.path) {
                self.serve_cached_content(
                    &cache_entry,
                    start,
                    end,
                    is_range_request,
                    if_modified_since,
                    is_head_request,
                    &resolved.path,
                )
            } else if resolved.is_virtual {
                // Handle virtual paths (like directory listings)
                if let Ok(html) = self.generate_directory_listing(&resolved.path).await {
                    let bytes = Bytes::from(html);
                    let content_length = bytes.len();
                    build_file_response(
                        StatusCode::OK,
                        "text/html",
                        content_length,
                        SystemTime::now(),
                        None,
                        BoxBody::new(Full::new(bytes)),
                    )
                } else {
                    Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .body(BoxBody::new(Full::new(Bytes::new())))
                        .unwrap()
                }
            } else {
                // Handle uncached files
                if let Some(body) = if is_range_request {
                    self.stream_file_range(resolved.path, start, end).await
                } else {
                    self.stream_file(resolved.path).await
                } {
                    Response::builder()
                        .status(StatusCode::OK)
                        .header("Content-Type", get_mime_type(&resolved.path))
                        .body(body)
                        .unwrap()
                } else {
                    Response::builder()
                        .status(StatusCode::NOT_FOUND)
                        .body(BoxBody::new(Full::new(Bytes::new())))
                        .unwrap()
                }
            }
        }
        Err(_) => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(BoxBody::new(Full::new(Bytes::new())))
            .unwrap(),
    }
} 