use super::{
    middleware_stack::ErrorResponse,
    types::{HttpRequest, HttpResponse},
};
use bytes::Bytes;
use chrono::{DateTime, Utc};
use http::{header, Response, StatusCode};
use http_body_util::{combinators::BoxBody, Full};
use itsi_error::Result;
use moka::sync::Cache;
use serde::Deserialize;
use std::{
    collections::HashMap,
    convert::Infallible,
    fs::Metadata,
    path::{Path, PathBuf},
    sync::{Arc, LazyLock},
    time::{Duration, Instant, SystemTime},
};
use tokio::sync::Mutex;
use tokio::{fs::File, io::AsyncReadExt};
use tracing::{info, warn};

pub static ROOT_STATIC_FILE_SERVER: LazyLock<StaticFileServer> = LazyLock::new(|| {
    StaticFileServer::new(StaticFileServerConfig {
        root_dir: Path::new("./").to_path_buf(),
        max_file_size: 4096,
        max_entries: 1024 * 1024 * 10,
        recheck_interval: Duration::from_secs(1),
        try_html_extension: true,
        auto_index: true,
        not_found_behavior: NotFoundBehavior::Error(ErrorResponse::default()),
    })
});

#[derive(Debug, Clone, Deserialize)]
pub struct Redirect {
    pub to: String,
}

#[derive(Debug, Clone, Deserialize)]
pub enum NotFoundBehavior {
    #[serde(rename = "error")]
    Error(ErrorResponse),
    #[serde(rename = "fallthrough")]
    FallThrough,
    #[serde(rename = "index")]
    IndexFile(PathBuf),
    #[serde(rename = "redirect")]
    Redirect(Redirect),
    #[serde(rename = "internal_server_error")]
    InternalServerError,
}

#[derive(Debug, Clone)]
pub struct StaticFileServerConfig {
    pub root_dir: PathBuf,
    pub max_file_size: u64,
    pub max_entries: u64,
    pub recheck_interval: Duration,
    pub try_html_extension: bool,
    pub auto_index: bool,
    pub not_found_behavior: NotFoundBehavior,
}

#[derive(Debug, Clone)]
pub struct StaticFileServer {
    config: Arc<StaticFileServerConfig>,
    key_to_path: Arc<Mutex<HashMap<String, PathBuf>>>,
    cache: Cache<PathBuf, CacheEntry>,
}

#[derive(Clone, Debug)]
struct CacheEntry {
    content: Arc<Bytes>,
    last_modified: SystemTime,
    last_checked: Instant,
}

#[derive(Debug, Clone)]
pub enum ServeRange {
    Range(u64, u64),
    Full,
}

impl CacheEntry {
    async fn new(path: PathBuf) -> Result<Self> {
        let (bytes, last_modified) = read_entire_file(&path).await?;
        Ok(CacheEntry {
            content: Arc::new(bytes),
            last_modified,
            last_checked: Instant::now(),
        })
    }

    async fn new_virtual_listing(path: PathBuf, root_dir: &Path) -> Self {
        let directory_listing: Bytes = generate_directory_listing(path.parent().unwrap(), root_dir)
            .await
            .unwrap_or("".to_owned())
            .into();
        CacheEntry {
            content: Arc::new(directory_listing),
            last_modified: SystemTime::now(),
            last_checked: Instant::now(),
        }
    }
}

impl StaticFileServer {
    pub fn new(config: StaticFileServerConfig) -> Self {
        let cache = Cache::builder().max_capacity(config.max_entries).build();

        StaticFileServer {
            config: Arc::new(config),
            cache,
            key_to_path: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn serve(
        &self,
        request: &HttpRequest,
        path: &str,
        abs_path: &str,
        serve_range: ServeRange,
        if_modified_since: Option<SystemTime>,
        is_head_request: bool,
    ) -> Option<HttpResponse> {
        let resolved = self.resolve(path, abs_path).await;
        Some(match resolved {
            Ok(ResolvedAsset {
                path,
                cache_entry,
                metadata,
                redirect_to: None,
            }) => {
                let (start, end) = match serve_range {
                    ServeRange::Full => (0, u64::MAX),
                    ServeRange::Range(start, end) => (start, end),
                };
                let is_range_request = matches!(serve_range, ServeRange::Range { .. });

                if let Some(cache_entry) = cache_entry {
                    self.serve_cached_content(
                        &cache_entry,
                        start,
                        end,
                        is_range_request,
                        if_modified_since,
                        is_head_request,
                        &path,
                    )
                } else {
                    self.serve_stream_content(
                        path,
                        metadata.unwrap(),
                        start,
                        end,
                        is_range_request,
                        if_modified_since,
                        is_head_request,
                    )
                    .await
                }
            }
            Ok(ResolvedAsset {
                redirect_to: Some(redirect_to),
                ..
            }) => Response::builder()
                .status(StatusCode::MOVED_PERMANENTLY)
                .header(header::LOCATION, redirect_to)
                .body(BoxBody::new(Full::new(Bytes::new())))
                .unwrap(),
            Err(not_found_behavior) => match not_found_behavior {
                NotFoundBehavior::Error(error_response) => {
                    error_response.to_http_response(request).await
                }
                NotFoundBehavior::FallThrough => return None,
                NotFoundBehavior::IndexFile(index_file) => {
                    self.serve_single(index_file.to_str().unwrap()).await
                }
                NotFoundBehavior::Redirect(redirect) => Response::builder()
                    .status(StatusCode::MOVED_PERMANENTLY)
                    .header(header::LOCATION, redirect.to)
                    .body(BoxBody::new(Full::new(Bytes::new())))
                    .unwrap(),
                NotFoundBehavior::InternalServerError => Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(BoxBody::new(Full::new(Bytes::new())))
                    .unwrap(),
            },
        })
    }

    pub async fn serve_single(&self, path: &str) -> HttpResponse {
        let resolved = self.resolve(path, path).await;
        info!("Resolved to {:?}", resolved);
        if let Ok(ResolvedAsset {
            path,
            cache_entry: Some(cache_entry),
            ..
        }) = resolved
        {
            return self.serve_cached_content(&cache_entry, 0, u64::MAX, false, None, false, &path);
        } else if let Ok(ResolvedAsset { path, metadata, .. }) = resolved {
            return self
                .serve_stream_content(path, metadata.unwrap(), 0, u64::MAX, false, None, false)
                .await;
        }

        Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(BoxBody::new(Full::new(Bytes::new())))
            .unwrap()
    }

    /// Resolves a request key to an actual file path and determines if it needs to be cached
    async fn resolve(
        &self,
        key: &str,
        abs_path: &str,
    ) -> std::result::Result<ResolvedAsset, NotFoundBehavior> {
        // First check if we have a cached mapping for this key
        if let Some(path) = self.key_to_path.lock().await.get(key) {
            // Check if the cached entry is still valid
            if let Some(entry) = self.cache.get(path) {
                let last_check_elapsed = entry.last_checked.elapsed();
                if last_check_elapsed < self.config.recheck_interval {
                    // Entry is still fresh, use it
                    return Ok(ResolvedAsset {
                        path: path.clone(),
                        cache_entry: Some(entry.clone()),
                        metadata: None,
                        redirect_to: None,
                    });
                }

                // Entry is stale, check if file has changed
                if let Ok(metadata) = tokio::fs::metadata(path).await {
                    if metadata
                        .modified()
                        .is_ok_and(|modified| modified == entry.last_modified)
                    {
                        // File hasn't changed, just update last_checked
                        let mut entry = entry;
                        entry.last_checked = Instant::now();
                        self.cache.insert(path.clone(), entry.clone());
                        return Ok(ResolvedAsset {
                            path: path.clone(),
                            cache_entry: Some(entry.clone()),
                            metadata: None,
                            redirect_to: None,
                        });
                    }

                    // File has changed, check if it's still cacheable
                    if metadata.len() > self.config.max_file_size {
                        // File is now too large, remove from cache
                        self.cache.invalidate(path);
                        self.key_to_path.lock().await.remove(key);
                    }
                }
            }
        }

        // No valid cached entry, resolve the key to a file path
        let normalized_path = normalize_path(key).ok_or(NotFoundBehavior::InternalServerError)?;
        let mut full_path = self.config.root_dir.clone();
        full_path.push(normalized_path);
        info!("Checking full path: {:?}", full_path);
        // Check if path exists and is a file
        match tokio::fs::metadata(&full_path).await {
            Ok(metadata) => {
                if metadata.is_file() {
                    let cache_entry = if metadata.len() <= self.config.max_file_size {
                        self.key_to_path
                            .lock()
                            .await
                            .insert(key.to_string(), full_path.clone());
                        let cache_entry = CacheEntry::new(full_path.clone()).await.unwrap();
                        self.cache.insert(full_path.clone(), cache_entry.clone());
                        Some(cache_entry)
                    } else {
                        None
                    };
                    info!("Returning resolved asset");
                    return Ok(ResolvedAsset {
                        path: full_path,
                        cache_entry,
                        metadata: Some(metadata),
                        redirect_to: None,
                    });
                } else if metadata.is_dir() {
                    if !abs_path.ends_with("/") {
                        return Ok(ResolvedAsset {
                            path: full_path,
                            cache_entry: None,
                            metadata: Some(metadata),
                            redirect_to: Some(format!("{}/", abs_path)),
                        });
                    }
                    let mut index_file = None;

                    let index_path = full_path.join("index.html");
                    if let Ok(idx_meta) = tokio::fs::metadata(&index_path).await {
                        if idx_meta.is_file() {
                            index_file = Some(index_path);
                        }
                    }

                    if index_file.is_none() {
                        // Check for case insensitive index.html
                        let entries = match tokio::fs::read_dir(&full_path).await {
                            Ok(entries) => entries,
                            Err(_) => return Err(NotFoundBehavior::InternalServerError),
                        };

                        tokio::pin!(entries);
                        while let Some(entry) = entries.next_entry().await.unwrap_or(None) {
                            if entry
                                .file_name()
                                .to_str()
                                .is_some_and(|name| name.eq_ignore_ascii_case("index.html"))
                                && entry.metadata().await.unwrap().is_file()
                            {
                                index_file = Some(entry.path());
                                break;
                            }
                        }
                    }
                    if index_file.is_some() {
                        let index_path = index_file.unwrap();
                        self.key_to_path
                            .lock()
                            .await
                            .insert(key.to_string(), index_path.clone());
                        let cache_entry = CacheEntry::new(index_path.clone()).await.unwrap();
                        self.cache.insert(index_path.clone(), cache_entry.clone());
                        return Ok(ResolvedAsset {
                            path: index_path,
                            cache_entry: Some(cache_entry),
                            metadata: None,
                            redirect_to: None,
                        });
                    }

                    // No index.html, check if auto_index is enabled
                    if self.config.auto_index {
                        // Create a virtual path for the directory listing
                        let virtual_path = full_path.join(".directory_listing.dir_list");

                        let cache_entry = CacheEntry::new_virtual_listing(
                            virtual_path.clone(),
                            &self.config.root_dir,
                        )
                        .await;
                        self.key_to_path
                            .lock()
                            .await
                            .insert(key.to_string(), virtual_path.clone());
                        self.cache.insert(virtual_path.clone(), cache_entry.clone());
                        return Ok(ResolvedAsset {
                            path: virtual_path.clone(),
                            cache_entry: Some(cache_entry.clone()),
                            metadata: None,
                            redirect_to: None,
                        });
                    }
                }
            }
            Err(_) => {
                // Path doesn't exist, try with .html extension if configured
                if self.config.try_html_extension {
                    let mut html_path = full_path.clone();
                    html_path.set_extension("html");

                    if let Ok(html_meta) = tokio::fs::metadata(&html_path).await {
                        if html_meta.is_file() {
                            self.key_to_path
                                .lock()
                                .await
                                .insert(key.to_string(), html_path.clone());
                            let cache_entry = if html_meta.len() <= self.config.max_file_size {
                                let cache_entry = CacheEntry::new(html_path.clone()).await.unwrap();
                                self.cache.insert(html_path.clone(), cache_entry.clone());
                                Some(cache_entry)
                            } else {
                                None
                            };
                            return Ok(ResolvedAsset {
                                path: html_path,
                                cache_entry,
                                metadata: Some(html_meta),
                                redirect_to: None,
                            });
                        }
                    }
                }
            }
        }

        // If we get here, we couldn't resolve the key to a file
        Err(self.config.not_found_behavior.clone())
    }

    async fn stream_file_range(
        &self,
        path: PathBuf,
        start: u64,
        end: u64,
    ) -> Option<BoxBody<Bytes, Infallible>> {
        use futures::TryStreamExt;
        use http_body_util::StreamBody;
        use hyper::body::Frame;
        use tokio::io::AsyncSeekExt;
        use tokio_util::io::ReaderStream;

        let mut file = match File::open(&path).await {
            Ok(f) => f,
            Err(e) => {
                warn!(
                    "Failed to open file for streaming: {}: {}",
                    path.display(),
                    e
                );
                return None;
            }
        };

        // Seek to the start position
        if let Err(e) = file.seek(std::io::SeekFrom::Start(start)).await {
            warn!(
                "Failed to seek to position {} in file {}: {}",
                start,
                path.display(),
                e
            );
            return None;
        }

        // Create a limited reader that will only read up to range_length bytes
        let range_length = end - start + 1;
        let limited_reader = tokio::io::AsyncReadExt::take(file, range_length);
        let path_clone = path.clone();
        let stream = ReaderStream::new(limited_reader)
            .map_ok(Frame::data)
            .map_err(move |e| {
                warn!("Error streaming file {}: {}", path_clone.display(), e);
                unreachable!("We handle IO errors above")
            });

        Some(BoxBody::new(StreamBody::new(stream)))
    }

    async fn stream_file(&self, path: PathBuf) -> Option<BoxBody<Bytes, Infallible>> {
        use futures::TryStreamExt;
        use http_body_util::StreamBody;
        use hyper::body::Frame;
        use tokio_util::io::ReaderStream;

        match File::open(&path).await {
            Ok(file) => {
                let path_clone = path.clone();
                let stream = ReaderStream::new(file)
                    .map_ok(Frame::data)
                    .map_err(move |e| {
                        warn!("Error streaming file {}: {}", path_clone.display(), e);
                        unreachable!("We handle IO errors above")
                    });
                Some(BoxBody::new(StreamBody::new(stream)))
            }
            Err(e) => {
                warn!(
                    "Failed to open file for streaming: {}: {}",
                    path.display(),
                    e
                );
                None
            }
        }
    }

    async fn serve_stream_content(
        &self,
        file: PathBuf,
        metadata: Metadata,
        start: u64,
        end: u64,
        is_range_request: bool,
        if_modified_since: Option<SystemTime>,
        is_head_request: bool,
    ) -> http::Response<BoxBody<Bytes, Infallible>> {
        let content_length = metadata.len();
        let last_modified = metadata.modified().unwrap();

        // Handle If-Modified-Since header
        if is_not_modified(last_modified, if_modified_since) {
            return build_not_modified_response();
        }

        // For range requests, validate the range bounds
        if is_range_request && start >= content_length {
            return Response::builder()
                .status(StatusCode::RANGE_NOT_SATISFIABLE)
                .header("Content-Range", format!("bytes */{}", content_length))
                .body(BoxBody::new(Full::new(Bytes::new())))
                .unwrap();
        }

        // Adjust end bound for open-ended ranges or to not exceed file size
        let adjusted_end = if end == u64::MAX {
            content_length - 1
        } else {
            std::cmp::min(end, content_length - 1)
        };

        // Create response based on request type
        let status = if is_range_request {
            StatusCode::PARTIAL_CONTENT
        } else {
            StatusCode::OK
        };

        let content_range = if is_range_request {
            Some(format!(
                "bytes {}-{}/{}",
                start, adjusted_end, content_length
            ))
        } else {
            None
        };

        // For HEAD requests, return just the headers
        if is_head_request {
            let mut builder = Response::builder()
                .status(status)
                .header("Content-Type", get_mime_type(&file))
                .header(
                    "Content-Length",
                    if is_range_request {
                        (adjusted_end - start + 1).to_string()
                    } else {
                        content_length.to_string()
                    },
                )
                .header("Last-Modified", format_http_date(last_modified));

            if let Some(range) = content_range {
                builder = builder.header("Content-Range", range);
            }

            return builder.body(BoxBody::new(Full::new(Bytes::new()))).unwrap();
        }

        // For GET requests, prepare the actual content
        if is_range_request {
            // Extract the requested range from the cached content
            let end_idx = std::cmp::min((adjusted_end + 1) as u64, content_length);

            build_file_response(
                status,
                get_mime_type(&file),
                (end_idx - start) as usize,
                last_modified,
                content_range,
                self.stream_file_range(file, start, end_idx).await.unwrap(),
            )
        } else {
            build_file_response(
                status,
                get_mime_type(&file),
                content_length as usize,
                last_modified,
                content_range,
                self.stream_file(file).await.unwrap(),
            )
        }
    }

    fn serve_cached_content(
        &self,
        cache_entry: &CacheEntry,
        start: u64,
        end: u64,
        is_range_request: bool,
        if_modified_since: Option<SystemTime>,
        is_head_request: bool,
        path: &Path,
    ) -> http::Response<BoxBody<Bytes, Infallible>> {
        let content_length = cache_entry.content.len() as u64;

        // Handle If-Modified-Since header
        if is_not_modified(cache_entry.last_modified, if_modified_since) {
            return build_not_modified_response();
        }

        // For range requests, validate the range bounds
        if is_range_request && start >= content_length {
            return Response::builder()
                .status(StatusCode::RANGE_NOT_SATISFIABLE)
                .header("Content-Range", format!("bytes */{}", content_length))
                .body(BoxBody::new(Full::new(Bytes::new())))
                .unwrap();
        }

        // Adjust end bound for open-ended ranges or to not exceed file size
        let adjusted_end = if end == u64::MAX {
            content_length - 1
        } else {
            std::cmp::min(end, content_length - 1)
        };

        // Create response based on request type
        let status = if is_range_request {
            StatusCode::PARTIAL_CONTENT
        } else {
            StatusCode::OK
        };

        let content_range = if is_range_request {
            Some(format!(
                "bytes {}-{}/{}",
                start, adjusted_end, content_length
            ))
        } else {
            None
        };

        // For HEAD requests, return just the headers
        if is_head_request {
            let mut builder = Response::builder()
                .status(status)
                .header("Content-Type", get_mime_type(path))
                .header(
                    "Content-Length",
                    if is_range_request {
                        (adjusted_end - start + 1).to_string()
                    } else {
                        content_length.to_string()
                    },
                )
                .header("Last-Modified", format_http_date(cache_entry.last_modified));

            if let Some(range) = content_range {
                builder = builder.header("Content-Range", range);
            }

            return builder.body(BoxBody::new(Full::new(Bytes::new()))).unwrap();
        }

        // For GET requests, prepare the actual content
        if is_range_request {
            // Extract the requested range from the cached content
            let start_idx = start as usize;
            let end_idx = std::cmp::min((adjusted_end + 1) as usize, cache_entry.content.len());
            let range_bytes = cache_entry.content.slice(start_idx..end_idx);

            build_file_response(
                status,
                get_mime_type(path),
                range_bytes.len(),
                cache_entry.last_modified,
                content_range,
                BoxBody::new(Full::new(range_bytes)),
            )
        } else {
            // Return the full content
            let content_clone = cache_entry.content.clone();
            let body = build_ok_body(content_clone);
            build_file_response(
                status,
                get_mime_type(path),
                content_length as usize,
                cache_entry.last_modified,
                content_range,
                body,
            )
        }
    }

    pub async fn invalidate_cache(&self, path: &Path) {
        if let Ok(path_buf) = path.to_path_buf().canonicalize() {
            self.cache.invalidate(&path_buf);
        }
    }
}

fn format_http_date(last_modified: SystemTime) -> String {
    let datetime = DateTime::<Utc>::from(last_modified);
    datetime.format("%a, %d %b %Y %H:%M:%S GMT").to_string()
}

async fn read_entire_file(path: &Path) -> std::io::Result<(Bytes, SystemTime)> {
    let metadata = tokio::fs::metadata(path).await?;
    let last_modified = metadata.modified()?;
    let mut file = File::open(path).await?;
    let mut buf = Vec::with_capacity(metadata.len().try_into().unwrap_or(4096));
    file.read_to_end(&mut buf).await?;
    Ok((Bytes::from(buf), last_modified))
}

fn build_ok_body(bytes: Arc<Bytes>) -> BoxBody<Bytes, Infallible> {
    BoxBody::new(Full::new(bytes.as_ref().clone()))
}

// Helper for mime type mapping from file extension
fn get_mime_type(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html") | Some("htm") => "text/html",
        Some("css") => "text/css",
        Some("js") => "application/javascript",
        Some("json") => "application/json",
        Some("txt") => "text/plain",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("ico") => "image/x-icon",
        Some("webp") => "image/webp",
        Some("pdf") => "application/pdf",
        Some("xml") => "application/xml",
        Some("zip") => "application/zip",
        Some("gz") => "application/gzip",
        Some("mp3") => "audio/mpeg",
        Some("mp4") => "video/mp4",
        Some("webm") => "video/webm",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("ttf") => "font/ttf",
        Some("otf") => "font/otf",
        Some("dir_list") => "text/html",
        _ => "application/octet-stream",
    }
}

// Helper function to handle not modified responses
fn build_not_modified_response() -> http::Response<BoxBody<Bytes, Infallible>> {
    Response::builder()
        .status(StatusCode::NOT_MODIFIED)
        .body(BoxBody::new(Full::new(Bytes::new())))
        .unwrap()
}

// Helper function to build a file response with common headers
fn build_file_response(
    status: StatusCode,
    content_type: &str,
    content_length: usize,
    last_modified: SystemTime,
    range_header: Option<String>,
    body: BoxBody<Bytes, Infallible>,
) -> http::Response<BoxBody<Bytes, Infallible>> {
    let mut builder = Response::builder()
        .status(status)
        .header("Content-Type", content_type)
        .header("Content-Length", content_length)
        .header("Last-Modified", format_http_date(last_modified));

    if let Some(range) = range_header {
        builder = builder.header("Content-Range", range);
    }

    builder.body(body).unwrap()
}

// Helper function to check if a file is too old based on If-Modified-Since
fn is_not_modified(last_modified: SystemTime, if_modified_since: Option<SystemTime>) -> bool {
    if let Some(ims) = if_modified_since {
        if ims >= last_modified {
            return true;
        }
    }
    false
}

fn normalize_path(path: &str) -> Option<PathBuf> {
    let mut normalized = PathBuf::new();
    let path = path.trim_start_matches('/');

    for segment in path.split('/') {
        if segment.is_empty() || segment == "." {
            continue;
        }

        if segment == ".." {
            return None;
        }

        // Reject Windows-style backslash separators just in case
        if segment.contains('\\') {
            return None;
        }

        normalized.push(segment);
    }

    Some(normalized)
}

#[derive(Debug)]
struct ResolvedAsset {
    path: PathBuf,
    cache_entry: Option<CacheEntry>,
    metadata: Option<Metadata>,
    redirect_to: Option<String>,
}

impl std::fmt::Display for StaticFileServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "StaticFileServer(root_dir: {:?})", self.config.root_dir)
    }
}

impl Default for StaticFileServer {
    fn default() -> Self {
        let config = StaticFileServerConfig {
            root_dir: "public".into(),
            max_file_size: 10 * 1024 * 1024,
            max_entries: 100,
            recheck_interval: Duration::from_secs(60),
            try_html_extension: true,
            auto_index: true,
            not_found_behavior: NotFoundBehavior::Error(ErrorResponse::default()),
        };
        Self::new(config)
    }
}

async fn generate_directory_listing(dir_path: &Path, root_dir: &Path) -> std::io::Result<String> {
    let mut html = String::new();

    html.push_str("<!DOCTYPE html>\n<html>\n<head>\n");
    html.push_str("<title>Directory listing for ");
    html.push_str(
        dir_path
            .strip_prefix(root_dir)
            .unwrap_or(Path::new(""))
            .to_str()
            .unwrap_or(""),
    );
    html.push_str("</title>\n");
    html.push_str("<meta charset=\"UTF-8\">\n");
    html.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n");
    html.push_str("<style>\n");
    html.push_str("  body { font-family: sans-serif; margin: 0; padding: 20px; }\n");
    html.push_str("  h1 { border-bottom: 1px solid #ccc; margin-top: 0; padding-bottom: 5px; }\n");
    html.push_str("  table { border-collapse: collapse; width: 100%; }\n");
    html.push_str("  th, td { text-align: left; padding: 8px; }\n");
    html.push_str("  tr:nth-child(even) { background-color: #f2f2f2; }\n");
    html.push_str("  th { background-color: #4CAF50; color: white; }\n");
    html.push_str("  a { text-decoration: none; color: #0366d6; }\n");
    html.push_str("  a:hover { text-decoration: underline; }\n");
    html.push_str("  .size { text-align: right; }\n");
    html.push_str("  .date { white-space: nowrap; }\n");
    html.push_str("</style>\n");
    html.push_str("</head>\n<body>\n");

    html.push_str("<h1>Directory listing for ");
    html.push_str(&dir_path.display().to_string());
    html.push_str("</h1>\n");

    html.push_str("<table>\n");
    html.push_str("<tr><th>Name</th><th>Size</th><th>Last Modified</th></tr>\n");

    // Add parent directory link if not in root

    if dir_path != root_dir {
        info!("{} != {}", dir_path.display(), root_dir.display());
        html.push_str("<tr><td><a href=\"..\">..</a></td><td class=\"size\">-</td><td class=\"date\">-</td></tr>\n");
    }

    // Read the directory entries
    let mut entries = tokio::fs::read_dir(dir_path).await.unwrap();
    let mut files = Vec::new();
    let mut dirs = Vec::new();

    while let Some(entry) = entries.next_entry().await? {
        let entry_path = entry.path();
        let metadata = entry.metadata().await?;
        let name = entry_path.file_name().unwrap().to_string_lossy();

        if metadata.is_dir() {
            dirs.push((name.to_string(), metadata));
        } else {
            files.push((name.to_string(), metadata));
        }
    }

    // Sort directories and files
    dirs.sort_by(|a, b| a.0.cmp(&b.0));
    files.sort_by(|a, b| a.0.cmp(&b.0));

    // Add directories to HTML
    for (name, metadata) in dirs {
        html.push_str("<tr>");
        html.push_str("<td><a href=\"");
        html.push_str(&name);
        html.push_str("/\">");
        html.push_str(&name);
        html.push_str("/</a></td>");
        html.push_str("<td class=\"size\">-</td>");

        if let Ok(modified) = metadata.modified() {
            let formatted_time = DateTime::<Utc>::from(modified)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string();
            html.push_str(&format!("<td class=\"date\">{}</td>", formatted_time));
        } else {
            html.push_str("<td class=\"date\">-</td>");
        }

        html.push_str("</tr>\n");
    }

    // Add files to HTML
    for (name, metadata) in files {
        html.push_str("<tr>");
        html.push_str("<td><a href=\"");
        html.push_str(&name);
        html.push_str("\">");
        html.push_str(&name);
        html.push_str("</a></td>");

        // Format file size
        let file_size = metadata.len();
        let formatted_size = if file_size < 1024 {
            format!("{} B", file_size)
        } else if file_size < 1024 * 1024 {
            format!("{:.1} KB", file_size as f64 / 1024.0)
        } else if file_size < 1024 * 1024 * 1024 {
            format!("{:.1} MB", file_size as f64 / (1024.0 * 1024.0))
        } else {
            format!("{:.1} GB", file_size as f64 / (1024.0 * 1024.0 * 1024.0))
        };

        html.push_str(&format!("<td class=\"size\">{}</td>", formatted_size));

        if let Ok(modified) = metadata.modified() {
            let formatted_time = DateTime::<Utc>::from(modified)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string();
            html.push_str(&format!("<td class=\"date\">{}</td>", formatted_time));
        } else {
            html.push_str("<td class=\"date\">-</td>");
        }

        html.push_str("</tr>\n");
    }

    html.push_str("</table>\n");
    html.push_str("<hr><p>Served by Itsi Static Assets</p>\n");
    html.push_str("</body>\n</html>");

    Ok(html)
}
