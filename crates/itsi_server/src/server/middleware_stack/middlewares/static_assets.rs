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
    HeaderMap, Method,
};
use itsi_error::ItsiError;
use magnus::error::Result;
use serde::Deserialize;
use std::{collections::HashMap, path::PathBuf, sync::OnceLock, time::Duration};

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
    pub serve_dot_files: bool,
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
        self.file_server
            .set(StaticFileServer::new(StaticFileServerConfig {
                root_dir: self.root_dir.clone(),
                not_found_behavior: self.not_found_behavior.clone(),
                auto_index: self.auto_index,
                max_entries: self.max_files_in_memory,
                try_html_extension: self.try_html_extension,
                max_file_size: self.max_file_size_in_memory,
                recheck_interval: Duration::from_secs(self.file_check_interval),
                serve_dot_files: self.serve_dot_files,
                allowed_extensions: self.allowed_extensions.clone(),
            }))
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
            return Ok(Either::Left(req));
        }
        let abs_path = req.uri().path();
        let rel_path = if !self.relative_path {
            abs_path
        } else {
            match context
                .matching_pattern
                .as_ref()
                .and_then(|pattern| pattern.captures(req.uri().path()))
                .and_then(|captures| captures.name("path_suffix"))
                .map(|m| m.as_str())
            {
                Some(suffix) => suffix,
                None => return Ok(Either::Left(req)),
            }
        };

        // Determine if this is a HEAD request
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
        let response = file_server
            .serve(
                &req,
                rel_path,
                abs_path,
                serve_range,
                if_modified_since,
                is_head_request,
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
    let range_header = headers.get(RANGE);
    if range_header.is_none() {
        return ServeRange::Full;
    }
    let range_header = range_header.unwrap().to_str().unwrap_or("");
    let bytes_prefix = "bytes=";
    if !range_header.starts_with(bytes_prefix) {
        return ServeRange::Full;
    }

    let range_str = &range_header[bytes_prefix.len()..];

    let range_parts: Vec<&str> = range_str
        .split(',')
        .next()
        .unwrap_or("")
        .split('-')
        .collect();
    if range_parts.len() != 2 {
        return ServeRange::Full;
    }

    let start = if range_parts[0].is_empty() {
        range_parts[1].parse::<u64>().unwrap_or(0)
    } else if let Ok(start) = range_parts[0].parse::<u64>() {
        start
    } else {
        return ServeRange::Full;
    };

    let end = if range_parts[1].is_empty() {
        u64::MAX // Use u64::MAX as sentinel for open-ended ranges
    } else if let Ok(end) = range_parts[1].parse::<u64>() {
        end // No conversion needed, already u64
    } else {
        return ServeRange::Full;
    };

    ServeRange::Range(start, end)
}

impl FromValue for StaticAssets {}
