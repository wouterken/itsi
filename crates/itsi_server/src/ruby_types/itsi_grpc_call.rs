use super::itsi_grpc_response_stream::ItsiGrpcResponseStream;
use crate::prelude::*;
use crate::server::{
    byte_frame::ByteFrame,
    itsi_service::RequestContext,
    request_job::RequestJob,
    types::{HttpRequest, HttpResponse},
};
use async_compression::futures::bufread::{GzipDecoder, GzipEncoder, ZlibDecoder, ZlibEncoder};
use bytes::Bytes;
use derive_more::Debug;
use futures::{executor::block_on, io::Cursor, AsyncReadExt};
use http::{request::Parts, Response, StatusCode};
use http_body_util::{combinators::BoxBody, BodyExt, Empty};
use itsi_error::from::CLIENT_CONNECTION_CLOSED;
use itsi_rb_helpers::{print_rb_backtrace, HeapValue};
use itsi_tracing::debug;
use magnus::{
    block::Proc,
    error::{ErrorType, Result as MagnusResult},
    Error, Symbol,
};
use magnus::{
    value::{LazyId, ReprValue},
    Ruby, Value,
};
use regex::Regex;
use std::sync::LazyLock;
use std::{collections::HashMap, sync::Arc, time::Instant};
use tokio::sync::mpsc::{self};

static ID_MESSAGE: LazyId = LazyId::new("message");
static MIN_GZIP_SIZE: u32 = 128;
static MIN_DEFLATE_SIZE: u32 = 128;
static METHOD_NAME_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"([a-z])([A-Z])").expect("Failed to compile regex"));

#[derive(Debug)]
#[magnus::wrap(class = "Itsi::GrpcCall", free_immediately, size)]
pub struct ItsiGrpcCall {
    pub parts: Parts,
    pub start: Instant,
    pub compression_in: CompressionAlgorithm,
    pub compression_out: CompressionAlgorithm,
    #[debug(skip)]
    pub context: RequestContext,
    #[debug(skip)]
    pub stream: ItsiGrpcResponseStream,
}

#[derive(Debug, Clone)]
pub enum CompressionAlgorithm {
    None,
    Deflate,
    Gzip,
}

impl ItsiGrpcCall {
    pub fn service_name(&self) -> MagnusResult<String> {
        let path = self.parts.uri.path();
        Ok(path.split('/').nth_back(1).unwrap().to_string())
    }

    pub fn method_name(&self) -> MagnusResult<Symbol> {
        let path = self.parts.uri.path();
        let method_name = path.split('/').nth_back(0).unwrap();
        let snake_case_method_name = METHOD_NAME_REGEX
            .replace_all(method_name, "${1}_${2}")
            .to_lowercase();
        Ok(Symbol::new(snake_case_method_name))
    }

    pub fn stream(&self) -> MagnusResult<ItsiGrpcResponseStream> {
        Ok(self.stream.clone())
    }

    pub fn timeout(&self) -> MagnusResult<Option<f64>> {
        let timeout_str = self
            .parts
            .headers
            .get("grpc-timeout")
            .and_then(|hv| hv.to_str().ok())
            .unwrap_or("");
        Ok(parse_grpc_timeout(timeout_str).ok())
    }

    pub fn is_cancelled(&self) -> MagnusResult<bool> {
        self.stream.is_cancelled()
    }

    pub fn add_headers(&self, headers: HashMap<Bytes, Vec<Bytes>>) -> MagnusResult<()> {
        self.stream.add_headers(headers)
    }

    pub fn content_type_str(&self) -> &str {
        self.parts
            .headers
            .get("Content-Type")
            .and_then(|hv| hv.to_str().ok())
            .unwrap_or("application/x-www-form-urlencoded")
    }

    pub fn is_json(&self) -> bool {
        self.content_type_str() == "application/json"
    }

    pub fn process(self, ruby: &Ruby, app_proc: Arc<HeapValue<Proc>>) -> magnus::error::Result<()> {
        let response = self.stream.clone();
        let result = app_proc.call::<_, Value>((self,));
        if let Err(err) = result {
            Self::internal_error(ruby, response, err);
        }
        Ok(())
    }

    pub fn internal_error(_ruby: &Ruby, stream: ItsiGrpcResponseStream, err: Error) {
        if let Some(rb_err) = err.value() {
            print_rb_backtrace(rb_err);
            stream.internal_server_error(err.to_string());
        } else {
            stream.internal_server_error(err.to_string());
        }
    }

    pub(crate) async fn process_request(
        app: Arc<HeapValue<Proc>>,
        hyper_request: HttpRequest,
        context: &RequestContext,
    ) -> itsi_error::Result<HttpResponse> {
        let (request, mut receiver) = ItsiGrpcCall::new(hyper_request, context).await;
        let shutdown_channel = context.service.shutdown_channel.clone();
        let response_stream = request.stream.clone();
        match context
            .sender
            .send(RequestJob::ProcessGrpcRequest(request, app))
            .await
        {
            Err(err) => {
                error!("Error occurred: {}", err);
                let mut response = Response::new(BoxBody::new(Empty::new()));
                *response.status_mut() = StatusCode::BAD_REQUEST;
                Ok(response)
            }
            _ => match receiver.recv().await {
                Some(first_frame) => Ok(response_stream
                    .build_response(first_frame, receiver, shutdown_channel)
                    .await),
                None => Ok(Response::new(BoxBody::new(Empty::new()))),
            },
        }
    }
    pub fn is_connection_closed_err(ruby: &Ruby, err: &Error) -> bool {
        match err.error_type() {
            ErrorType::Jump(_) => false,
            ErrorType::Error(_, _) => false,
            ErrorType::Exception(exception) => {
                exception.is_kind_of(ruby.exception_eof_error())
                    && err
                        .value()
                        .map(|v| {
                            v.funcall::<_, _, String>(*ID_MESSAGE, ())
                                .unwrap_or("".to_string())
                                .eq(CLIENT_CONNECTION_CLOSED)
                        })
                        .unwrap_or(false)
            }
        }
    }

    pub fn should_compress_output(&self, message_size: u32) -> bool {
        match self.compression_out {
            CompressionAlgorithm::Gzip => message_size > MIN_GZIP_SIZE,
            CompressionAlgorithm::Deflate => message_size > MIN_DEFLATE_SIZE,
            CompressionAlgorithm::None => false,
        }
    }

    pub fn compress_output(&self, bytes: Bytes) -> MagnusResult<Bytes> {
        match self.compression_out {
            CompressionAlgorithm::Gzip => Self::compress_gzip(bytes),
            CompressionAlgorithm::Deflate => Self::compress_deflate(bytes),
            CompressionAlgorithm::None => Ok(bytes),
        }
    }

    pub fn decompress_input(&self, bytes: Bytes) -> MagnusResult<Bytes> {
        match self.compression_in {
            CompressionAlgorithm::Gzip => Self::decompress_gzip(bytes),
            CompressionAlgorithm::Deflate => Self::decompress_deflate(bytes),
            CompressionAlgorithm::None => Ok(bytes),
        }
    }

    fn decompress_deflate(input: Bytes) -> MagnusResult<Bytes> {
        let cursor = Cursor::new(input);
        let mut decoder = ZlibDecoder::new(cursor);

        let result = block_on(async {
            let mut output = Vec::new();
            decoder.read_to_end(&mut output).await?;
            Ok(Bytes::from(output))
        })
        .map_err(|e: std::io::Error| {
            Error::new(
                magnus::exception::standard_error(),
                format!("deflate decompression failed: {}", e),
            )
        })?;

        Ok(result)
    }

    fn decompress_gzip(input: Bytes) -> MagnusResult<Bytes> {
        let cursor = Cursor::new(input);
        let mut decoder = GzipDecoder::new(cursor);

        let result = block_on(async {
            let mut output = Vec::new();
            decoder.read_to_end(&mut output).await?;
            Ok(Bytes::from(output))
        })
        .map_err(|e: std::io::Error| {
            Error::new(
                magnus::exception::standard_error(),
                format!("gzip decompression failed: {}", e),
            )
        })?;

        Ok(result)
    }

    fn compress_gzip(input: Bytes) -> MagnusResult<Bytes> {
        let mut output = Vec::with_capacity(input.len() / 2);
        let cursor = Cursor::new(input);
        let mut encoder = GzipEncoder::new(cursor);

        let result = block_on(async {
            encoder.read_to_end(&mut output).await?;
            Ok::<Bytes, std::io::Error>(output.into())
        })
        .map_err(|e| {
            Error::new(
                magnus::exception::standard_error(),
                format!("gzip compression failed: {e}"),
            )
        })?;

        Ok(result)
    }

    fn compress_deflate(input: Bytes) -> MagnusResult<Bytes> {
        let mut output = Vec::with_capacity(input.len() / 2);
        let cursor = Cursor::new(input);
        let mut encoder = ZlibEncoder::new(cursor);

        let result = block_on(async {
            encoder.read_to_end(&mut output).await?;
            Ok::<Bytes, std::io::Error>(output.into())
        })
        .map_err(|e| {
            Error::new(
                magnus::exception::standard_error(),
                format!("deflate compression failed: {e}"),
            )
        })?;

        Ok(result)
    }

    pub(crate) async fn new(
        request: HttpRequest,
        context: &RequestContext,
    ) -> (ItsiGrpcCall, mpsc::Receiver<ByteFrame>) {
        let (parts, body) = request.into_parts();
        let response_channel = mpsc::channel::<ByteFrame>(100);
        let compression_in: CompressionAlgorithm = match parts.headers.get("grpc-encoding") {
            Some(encoding) => match encoding.to_str() {
                Ok(encoding) => match encoding {
                    "gzip" => CompressionAlgorithm::Gzip,
                    "deflate" => CompressionAlgorithm::Deflate,
                    _ => CompressionAlgorithm::None,
                },
                Err(_) => CompressionAlgorithm::None,
            },
            None => CompressionAlgorithm::None,
        };
        let compression_out: CompressionAlgorithm = match parts.headers.get("grpc-accept-encoding")
        {
            Some(accept_encoding) => match accept_encoding.to_str() {
                Ok(accept_encoding) => {
                    let encodings: Vec<&str> =
                        accept_encoding.split(',').map(|s| s.trim()).collect();
                    if encodings.contains(&"gzip") {
                        CompressionAlgorithm::Gzip
                    } else if encodings.contains(&"deflate") {
                        CompressionAlgorithm::Deflate
                    } else {
                        CompressionAlgorithm::None
                    }
                }
                Err(_) => CompressionAlgorithm::None,
            },
            None => CompressionAlgorithm::None,
        };
        (
            Self {
                context: context.clone(),
                start: Instant::now(),
                compression_out: compression_out.clone(),
                compression_in,
                parts,
                stream: ItsiGrpcResponseStream::new(
                    compression_out,
                    response_channel.0,
                    body.into_data_stream(),
                )
                .await,
            },
            response_channel.1,
        )
    }
}

fn parse_grpc_timeout(timeout_str: &str) -> Result<f64, &'static str> {
    if timeout_str.len() < 2 {
        return Err("Timeout string too short");
    }
    let (value_str, unit) = timeout_str.split_at(timeout_str.len() - 1);
    let value: u64 = value_str.parse().map_err(|_| "Invalid timeout value")?;
    let duration_secs = match unit {
        "n" => value as f64 / 1_000_000_000.0, // nanoseconds
        "u" => value as f64 / 1_000_000.0,     // microseconds
        "m" => value as f64 / 1_000.0,         // milliseconds
        "S" => value as f64,                   // seconds
        "M" => value as f64 * 60.0,            // minutes
        "H" => value as f64 * 3600.0,          // hours
        _ => return Err("Invalid timeout unit"),
    };

    Ok(duration_secs)
}
