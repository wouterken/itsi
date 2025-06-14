use crate::{
    server::http_message_types::{HttpBody, HttpRequest, HttpResponse},
    services::itsi_http_service::HttpRequestContext,
};

use super::{
    header_interpretation::{find_first_supported, header_contains},
    FromValue, MiddlewareLayer,
};

use async_compression::{
    tokio::bufread::{BrotliEncoder, DeflateEncoder, GzipEncoder, ZstdEncoder},
    Level,
};
use async_trait::async_trait;
use bytes::{Bytes, BytesMut};
use either::Either;
use futures::TryStreamExt;
use http::{
    header::{GetAll, CONTENT_ENCODING, CONTENT_LENGTH, CONTENT_TYPE},
    HeaderValue, Response,
};
use http_body_util::{BodyExt, StreamBody};
use hyper::body::Body;
use magnus::error::Result;
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use tokio::io::{AsyncRead, AsyncReadExt, BufReader};
use tokio_stream::StreamExt;
use tokio_util::io::{ReaderStream, StreamReader};
use tracing::debug;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Compression {
    min_size: usize,
    algorithms: Vec<CompressionAlgorithm>,
    compress_streams: bool,
    mime_types: Vec<MimeType>,
    level: CompressionLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum CompressionLevel {
    #[serde(rename(deserialize = "fastest"))]
    Fastest,
    #[serde(rename(deserialize = "best"))]
    Best,
    #[serde(rename(deserialize = "balanced"))]
    Balanced,
    #[serde(rename(deserialize = "precise"))]
    Precise(i32),
}

impl CompressionLevel {
    fn to_async_compression_level(&self) -> Level {
        match self {
            CompressionLevel::Fastest => Level::Fastest,
            CompressionLevel::Best => Level::Best,
            CompressionLevel::Balanced => Level::Default,
            CompressionLevel::Precise(level) => Level::Precise(*level),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, PartialOrd, Eq, Ord)]
pub enum CompressionAlgorithm {
    #[serde(rename(deserialize = "gzip"))]
    Gzip,
    #[serde(rename(deserialize = "br"))]
    Brotli,
    #[serde(rename(deserialize = "deflate"))]
    Deflate,
    #[serde(rename(deserialize = "zstd"))]
    Zstd,
    #[serde(rename(deserialize = "identity"))]
    Identity,
}

impl CompressionAlgorithm {
    pub fn as_str(&self) -> &'static str {
        match self {
            CompressionAlgorithm::Gzip => "gzip",
            CompressionAlgorithm::Brotli => "br",
            CompressionAlgorithm::Deflate => "deflate",
            CompressionAlgorithm::Zstd => "zstd",
            CompressionAlgorithm::Identity => "identity",
        }
    }

    pub fn header_value(&self) -> HeaderValue {
        HeaderValue::from_str(self.as_str()).unwrap()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum MimeType {
    #[serde(rename(deserialize = "text"))]
    Text,
    #[serde(rename(deserialize = "image"))]
    Image,
    #[serde(rename(deserialize = "application"))]
    Application,
    #[serde(rename(deserialize = "audio"))]
    Audio,
    #[serde(rename(deserialize = "video"))]
    Video,
    #[serde(rename(deserialize = "font"))]
    Font,
    #[serde(rename(deserialize = "other"))]
    Other(String),
    #[serde(rename(deserialize = "all"))]
    All,
}

impl MimeType {
    pub fn matches(&self, content_encodings: &GetAll<HeaderValue>) -> bool {
        match self {
            MimeType::Text => header_contains(content_encodings, "text/*"),
            MimeType::Image => header_contains(content_encodings, "image/*"),
            MimeType::Application => header_contains(content_encodings, "application/*"),
            MimeType::Audio => header_contains(content_encodings, "audio/*"),
            MimeType::Video => header_contains(content_encodings, "video/*"),
            MimeType::Font => header_contains(content_encodings, "font/*"),
            MimeType::Other(v) => header_contains(content_encodings, v),
            MimeType::All => header_contains(content_encodings, "*"),
        }
    }
}

fn stream_encode<R>(encoder: R) -> HttpBody
where
    R: AsyncRead + Unpin + Sync + Send + 'static,
{
    let encoded_stream = ReaderStream::new(encoder)
        .map(|res| res.map_err(|_| -> Infallible { unreachable!("We handle IO errors above") }));
    HttpBody::stream(StreamBody::new(encoded_stream))
}

fn update_content_encoding(parts: &mut http::response::Parts, new_encoding: HeaderValue) {
    if let Some(existing) = parts.headers.get(CONTENT_ENCODING) {
        let mut encodings = existing.to_str().unwrap_or("").to_owned();
        if !encodings.is_empty() {
            encodings.push_str(", ");
        }
        encodings.push_str(new_encoding.to_str().unwrap());
        parts
            .headers
            .insert(CONTENT_ENCODING, HeaderValue::from_str(&encodings).unwrap());
    } else {
        parts.headers.insert(CONTENT_ENCODING, new_encoding);
    }
}

#[async_trait]
impl MiddlewareLayer for Compression {
    async fn before(
        &self,
        req: HttpRequest,
        context: &mut HttpRequestContext,
    ) -> Result<Either<HttpRequest, HttpResponse>> {
        context.set_supported_encoding_set(&req);
        Ok(Either::Left(req))
    }

    /// We'll apply compression on the response, where appropriate.
    /// This is if:
    /// * The response body is larger than the minimum size.
    /// * The response content type is supported.
    /// * The client supports the compression algorithm.
    async fn after(&self, resp: HttpResponse, context: &mut HttpRequestContext) -> HttpResponse {
        let body_size = resp.size_hint().exact();
        let resp = resp;

        // Don't compress if it's not an explicitly listed compressable type
        if !self
            .mime_types
            .iter()
            .any(|mt| mt.matches(&resp.headers().get_all(CONTENT_TYPE)))
        {
            debug!(
                target: "middleware::compress",
                "Mime type not supported for compression"
            );
            return resp;
        }

        // Don't compress streams unless compress streams is enabled.
        if body_size.is_none() && !self.compress_streams {
            debug!(
                target: "middleware::compress",
                "Stream compression disabled"
            );
            return resp;
        }

        // Don't compress too small bodies
        if body_size.is_some_and(|s| s < self.min_size as u64) {
            debug!(
                target: "middleware::compress",
                "Body size too small for compression"
            );
            return resp;
        }

        // Don't recompress if we're already compressed in a supported format
        for existing_encoding in resp.headers().get_all(CONTENT_ENCODING) {
            if let Ok(encodings) = existing_encoding.to_str() {
                for encoding in encodings.split(',').map(str::trim) {
                    let encoding = encoding.split(';').next().unwrap_or(encoding).trim();
                    if self.algorithms.iter().any(|algo| algo.as_str() == encoding) {
                        debug!(
                            target: "middleware::compress",
                            "Body already compressed with supported algorithm"
                        );
                        return resp;
                    }
                }
            }
        }

        let compression_method =
            if let Some(supported_encoding_set) = context.supported_encoding_set() {
                match find_first_supported(
                    supported_encoding_set,
                    self.algorithms.iter().map(|algo| algo.as_str()),
                ) {
                    Some("gzip") => CompressionAlgorithm::Gzip,
                    Some("br") => CompressionAlgorithm::Brotli,
                    Some("deflate") => CompressionAlgorithm::Deflate,
                    Some("zstd") => CompressionAlgorithm::Zstd,
                    _ => CompressionAlgorithm::Identity,
                }
            } else {
                CompressionAlgorithm::Identity
            };

        debug!(
            target: "middleware::compress",
            "Selected compression method: {:?}", compression_method
        );
        if matches!(compression_method, CompressionAlgorithm::Identity) {
            return resp;
        }

        let (mut parts, body) = resp.into_parts();

        let new_body = if let Some(_size) = body_size {
            let full_bytes: Bytes = body
                .into_data_stream()
                .try_fold(BytesMut::new(), |mut acc, chunk| async move {
                    acc.extend_from_slice(&chunk);
                    Ok(acc)
                })
                .await
                .unwrap()
                .freeze();

            let cursor = std::io::Cursor::new(full_bytes);
            let reader = BufReader::new(cursor);
            let compressed_bytes = match compression_method {
                CompressionAlgorithm::Gzip => {
                    let mut encoder =
                        GzipEncoder::with_quality(reader, self.level.to_async_compression_level());
                    let mut buf = Vec::new();
                    encoder.read_to_end(&mut buf).await.unwrap();
                    buf
                }
                CompressionAlgorithm::Brotli => {
                    let mut encoder = BrotliEncoder::with_quality(
                        reader,
                        self.level.to_async_compression_level(),
                    );
                    let mut buf = Vec::new();
                    encoder.read_to_end(&mut buf).await.unwrap();
                    buf
                }
                CompressionAlgorithm::Deflate => {
                    let mut encoder = DeflateEncoder::with_quality(
                        reader,
                        self.level.to_async_compression_level(),
                    );
                    let mut buf = Vec::new();
                    encoder.read_to_end(&mut buf).await.unwrap();
                    buf
                }
                CompressionAlgorithm::Zstd => {
                    let mut encoder =
                        ZstdEncoder::with_quality(reader, self.level.to_async_compression_level());
                    let mut buf = Vec::new();
                    encoder.read_to_end(&mut buf).await.unwrap();
                    buf
                }
                CompressionAlgorithm::Identity => unreachable!(),
            };
            HttpBody::full(Bytes::from(compressed_bytes))
        } else {
            let stream = body
                .into_data_stream()
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e));
            let async_read_fut = StreamReader::new(stream);
            let reader = BufReader::new(async_read_fut);
            match compression_method {
                CompressionAlgorithm::Gzip => stream_encode(GzipEncoder::with_quality(
                    reader,
                    self.level.to_async_compression_level(),
                )),
                CompressionAlgorithm::Brotli => stream_encode(BrotliEncoder::with_quality(
                    reader,
                    self.level.to_async_compression_level(),
                )),
                CompressionAlgorithm::Deflate => stream_encode(DeflateEncoder::with_quality(
                    reader,
                    self.level.to_async_compression_level(),
                )),
                CompressionAlgorithm::Zstd => stream_encode(ZstdEncoder::with_quality(
                    reader,
                    self.level.to_async_compression_level(),
                )),
                CompressionAlgorithm::Identity => unreachable!(),
            }
        };

        update_content_encoding(&mut parts, compression_method.header_value());
        parts.headers.remove(CONTENT_LENGTH);
        debug!(
            target: "middleware::compress",
            "Response compressed"
        );
        Response::from_parts(parts, new_body)
    }
}
impl FromValue for Compression {}
