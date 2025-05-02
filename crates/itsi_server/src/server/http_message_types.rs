use std::convert::Infallible;

use bytes::Bytes;
use http::{Request, Response};
use http_body_util::combinators::BoxBody;
use hyper::body::Incoming;

use super::size_limited_incoming::SizeLimitedIncoming;

pub type HttpResponse = Response<BoxBody<Bytes, Infallible>>;
pub type HttpRequest = Request<SizeLimitedIncoming<Incoming>>;

pub trait ConversionExt {
    fn limit(self) -> HttpRequest;
}

impl ConversionExt for Request<Incoming> {
    fn limit(self) -> HttpRequest {
        let (parts, body) = self.into_parts();
        Request::from_parts(parts, SizeLimitedIncoming::new(body))
    }
}

pub trait RequestExt {
    fn content_type(&self) -> Option<&str>;
    fn accept(&self) -> Option<&str>;
    fn header(&self, header_name: &str) -> Option<&str>;
    fn query_param(&self, query_name: &str) -> Option<&str>;
}

pub trait PathExt {
    fn no_trailing_slash(&self) -> &str;
}

#[derive(Debug, Clone, Copy)]
pub enum ResponseFormat {
    JSON,
    HTML,
    TEXT,
    UNKNOWN,
}

#[derive(Debug, Clone, Default)]
pub struct SupportedEncodingSet {
    pub zstd: bool,
    pub br: bool,
    pub deflate: bool,
    pub gzip: bool,
}

impl From<Option<&str>> for ResponseFormat {
    fn from(value: Option<&str>) -> Self {
        match value {
            Some("application/json") => ResponseFormat::JSON,
            Some("text/html") => ResponseFormat::HTML,
            Some("text/plain") => ResponseFormat::TEXT,
            _ => ResponseFormat::UNKNOWN,
        }
    }
}

impl PathExt for str {
    fn no_trailing_slash(&self) -> &str {
        if self == "/" {
            self
        } else {
            self.trim_end_matches("/")
        }
    }
}

impl RequestExt for HttpRequest {
    fn content_type(&self) -> Option<&str> {
        self.headers()
            .get("content-type")
            .map(|hv| hv.to_str().unwrap_or(""))
    }

    fn accept(&self) -> Option<&str> {
        self.headers()
            .get("accept")
            .map(|hv| hv.to_str().unwrap_or(""))
    }

    fn header(&self, header_name: &str) -> Option<&str> {
        self.headers()
            .get(header_name)
            .map(|hv| hv.to_str().unwrap_or(""))
    }

    fn query_param(&self, query_name: &str) -> Option<&str> {
        self.uri()
            .query()
            .and_then(|query| query.split('&').find(|param| param.starts_with(query_name)))
            .map(|param| param.split('=').nth(1).unwrap_or(""))
    }
}
