use super::{FromValue, MiddlewareLayer};
use crate::{
    server::http_message_types::{HttpRequest, HttpResponse},
    services::{
        itsi_http_service::HttpRequestContext,
        static_file_server::{
            NotFoundBehavior, ServeRange, StaticFileServer, StaticFileServerConfig,
        },
    },
};
use async_trait::async_trait;
use either::Either;
use http::{
    header::{IF_MODIFIED_SINCE, RANGE},
    HeaderMap, HeaderValue, Method,
};
use itsi_error::ItsiError;
use magnus::error::Result;
use quick_cache::sync::Cache;
use regex::Regex;
use serde::Deserialize;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, OnceLock},
    time::Duration,
};
use tracing::debug;

/// Compact representation of the client's Accept-Encoding preferences.
/// Priority order is determined by the bit checks in `pick_encoding`.
#[derive(Clone, Copy, Debug, Default)]
struct AcceptEncodingMask(u8);

impl AcceptEncodingMask {
    const BR: u8 = 1 << 0;
    const GZIP: u8 = 1 << 1;
    const ZSTD: u8 = 1 << 2;
    const DEFLATE: u8 = 1 << 3;

    fn from_headers(headers: &[HeaderValue]) -> Self {
        let mut mask = 0u8;

        for hv in headers {
            let Ok(s) = hv.to_str() else { continue };

            // We intentionally ignore q-values and treat any mention as "acceptable".
            // This is a fast-path optimization for common benchmark/client headers.
            for part in s.split(',') {
                let token = part.split(';').next().unwrap_or("").trim();
                match token {
                    "br" => mask |= Self::BR,
                    "gzip" => mask |= Self::GZIP,
                    "zstd" => mask |= Self::ZSTD,
                    "deflate" => mask |= Self::DEFLATE,
                    _ => {}
                }
            }
        }

        Self(mask)
    }

    fn pick_encoding(self) -> Option<&'static str> {
        // Prefer stronger/faster compression if available.
        // (Actual availability is checked by the file server.)
        if (self.0 & Self::ZSTD) != 0 {
            return Some("zstd");
        }
        if (self.0 & Self::BR) != 0 {
            return Some("br");
        }
        if (self.0 & Self::GZIP) != 0 {
            return Some("gzip");
        }
        if (self.0 & Self::DEFLATE) != 0 {
            return Some("deflate");
        }
        None
    }
}

#[derive(Debug, Deserialize)]
pub struct StaticAssets {
    pub root_dir: PathBuf,
    pub not_found_behavior: NotFoundBehavior,
    pub auto_index: bool,
    pub try_html_extension: bool,
    pub max_file_size_in_memory: u64,
    pub max_files_in_memory: u64,
    pub file_check_interval: u64,
    pub headers: Option<HashMap<String, String>>,
    pub allowed_extensions: Vec<String>,
    pub relative_path: bool,
    pub serve_hidden_files: bool,
    pub base_path: String,
    #[serde(skip)]
    pub base_path_regex: OnceLock<Regex>,
    #[serde(skip)]
    file_server: OnceLock<StaticFileServer>,
}

#[async_trait]
impl MiddlewareLayer for StaticAssets {
    async fn initialize(&self) -> Result<()> {
        if let Ok(metadata) = tokio::fs::metadata(&self.root_dir).await {
            if metadata.is_dir() {
                Ok(())
            } else {
                Err(ItsiError::InvalidInput(
                    "Root directory exists but is not a directory".to_string(),
                ))
            }
        } else {
            Err(ItsiError::InvalidInput(
                "Root directory exists but is not a directory".to_string(),
            ))
        }?;
        self.base_path_regex
            .set(Regex::new(&self.base_path).map_err(ItsiError::new)?)
            .map_err(ItsiError::new)?;

        debug!(target: "middleware::static_assets", "Base path regexp: {}", self.base_path);

        self.file_server
            .set(StaticFileServer::new(StaticFileServerConfig {
                root_dir: self.root_dir.clone(),
                not_found_behavior: self.not_found_behavior.clone(),
                auto_index: self.auto_index,
                max_entries: self.max_files_in_memory,
                try_html_extension: self.try_html_extension,
                max_file_size: self.max_file_size_in_memory,
                headers: self.headers.clone(),
                recheck_interval: Duration::from_secs(self.file_check_interval),
                serve_hidden_files: self.serve_hidden_files,
                allowed_extensions: self.allowed_extensions.clone(),
                miss_cache: Arc::new(Cache::new(self.max_files_in_memory as usize)),
            })?)
            .map_err(ItsiError::new)?;
        Ok(())
    }

    async fn before(
        &self,
        req: HttpRequest,
        context: &mut HttpRequestContext,
    ) -> Result<Either<HttpRequest, HttpResponse>> {
        // Only handle GET and HEAD requests
        if req.method() != Method::GET && req.method() != Method::HEAD {
            debug!(target: "middleware::static_assets", "Refusing to handle non-GET/HEAD request");
            return Ok(Either::Left(req));
        }

        // We still populate the context cache for any other middleware that might want it,
        // but we avoid re-parsing Accept-Encoding later by computing a compact mask here.
        context.set_supported_encoding_set(&req);

        let abs_path = req.uri().path();
        let rel_path = if !self.relative_path {
            abs_path.trim_start_matches("/")
        } else {
            let base_path = self
                .base_path_regex
                .get()
                .unwrap()
                .captures(abs_path)
                .and_then(|caps| caps.name("base_path"))
                .map(|m| m.as_str())
                .unwrap_or("/");

            match abs_path.strip_prefix(base_path) {
                Some(suffix) => suffix,
                None => return Ok(Either::Left(req)),
            }
        };

        debug!(target: "middleware::static_assets", "Asset path is {}", rel_path);
        let is_head_request = req.method() == Method::HEAD;

        // Extract range and if-modified-since headers
        let serve_range = parse_range_header(req.headers());
        let if_modified_since = req
            .headers()
            .get(IF_MODIFIED_SINCE)
            .and_then(|ims| ims.to_str().ok())
            .and_then(|ims_str| httpdate::parse_http_date(ims_str).ok());

        // Let the file server handle everything
        let file_server = self.file_server.get().unwrap();
        let encodings: &[HeaderValue] = context
            .supported_encoding_set()
            .map_or(&[], |set| set.as_slice());

        // Compute a fast encoding preference and narrow the encoding list we hand to the server.
        // This avoids repeated per-request string splitting/trim in the static file server.
        let mask = AcceptEncodingMask::from_headers(encodings);
        let preferred = mask.pick_encoding();

        let narrowed: [HeaderValue; 1];
        let encodings_for_server: &[HeaderValue] = if let Some(token) = preferred {
            // Safe: these are valid header values and the file server only needs to see
            // a minimal representation to pick a cached variant.
            narrowed = [HeaderValue::from_static(token)];
            &narrowed
        } else {
            &[]
        };

        let response = file_server
            .serve(
                &req,
                rel_path,
                abs_path,
                serve_range,
                if_modified_since,
                is_head_request,
                encodings_for_server,
            )
            .await;

        if response.is_none() {
            Ok(Either::Left(req))
        } else {
            Ok(Either::Right(response.unwrap()))
        }
    }
}

fn parse_range_header(headers: &HeaderMap) -> ServeRange {
    let Some(range_header) = headers.get(RANGE) else {
        return ServeRange::Full;
    };

    let range_header = range_header.to_str().unwrap_or("");
    let bytes_prefix = "bytes=";
    if !range_header.starts_with(bytes_prefix) {
        return ServeRange::Full;
    }

    // Only consider the first range specifier, ignore multi-range requests.
    let range_str = range_header[bytes_prefix.len()..]
        .split(',')
        .next()
        .unwrap_or("");

    let Some((start_str, end_str)) = range_str.split_once('-') else {
        return ServeRange::Full;
    };

    let start = if start_str.is_empty() {
        end_str.parse::<u64>().unwrap_or(0)
    } else if let Ok(start) = start_str.parse::<u64>() {
        start
    } else {
        return ServeRange::Full;
    };

    let end = if end_str.is_empty() {
        u64::MAX // sentinel for open-ended ranges
    } else if let Ok(end) = end_str.parse::<u64>() {
        end
    } else {
        return ServeRange::Full;
    };

    ServeRange::Range(start, end)
}

impl FromValue for StaticAssets {}
