use bytes::Bytes;
use core::fmt;
use futures::Stream;
use futures_util::TryStreamExt;
use http::Request;
use http_body_util::{combinators::WithTrailers, BodyExt, Either, Empty, Full, StreamBody};
use hyper::body::{Body, Frame, Incoming, SizeHint};
use std::{
    convert::Infallible,
    pin::Pin,
    task::{Context, Poll},
};

use super::size_limited_incoming::SizeLimitedIncoming;

type Inner = Either<Full<Bytes>, Empty<Bytes>>;

type BoxStream =
    Pin<Box<dyn Stream<Item = Result<Frame<Bytes>, Infallible>> + Send + Sync + 'static>>;

pub struct PlainBody(Either<StreamBody<BoxStream>, Inner>);

impl fmt::Debug for PlainBody {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            Either::Left(_) => f.write_str("PlainBody::Stream(..)"),
            Either::Right(inner) => match inner {
                Either::Left(full) => f.debug_tuple("PlainBody::Full").field(full).finish(),
                Either::Right(_) => f.write_str("PlainBody::Empty"),
            },
        }
    }
}
type DynErr = Box<dyn std::error::Error + Send + Sync>;

impl Body for PlainBody {
    type Data = Bytes;
    type Error = DynErr;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        unsafe { self.map_unchecked_mut(|s| &mut s.0) }.poll_frame(cx)
    }

    fn size_hint(&self) -> SizeHint {
        self.0.size_hint()
    }
}

impl PlainBody {
    fn stream<S>(s: S) -> Self
    where
        S: Stream<Item = Result<Bytes, Infallible>> + Send + Sync + 'static,
    {
        let boxed: BoxStream = Box::pin(s.map_ok(Frame::data));
        Self(Either::Left(StreamBody::new(boxed)))
    }

    fn full(bytes: Bytes) -> Self {
        Self(Either::Right(Either::Left(Full::new(bytes))))
    }

    fn empty() -> Self {
        Self(Either::Right(Either::Right(Empty::new())))
    }
}

type BoxTrailers = Pin<
    Box<dyn std::future::Future<Output = Option<Result<http::HeaderMap, DynErr>>> + Send + Sync>,
>;

pub enum HttpBody {
    Plain(PlainBody),
    WithT(WithTrailers<PlainBody, BoxTrailers>),
}

impl fmt::Debug for HttpBody {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HttpBody::Plain(b) => f.debug_tuple("HttpBody::Plain").field(b).finish(),
            HttpBody::WithT(_) => f.write_str("HttpBody::WithT(..)"),
        }
    }
}

impl Body for HttpBody {
    type Data = Bytes;
    type Error = DynErr;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        unsafe {
            match self.get_unchecked_mut() {
                HttpBody::Plain(b) => Pin::new_unchecked(b).poll_frame(cx),
                HttpBody::WithT(b) => Pin::new_unchecked(b).poll_frame(cx),
            }
        }
    }

    fn size_hint(&self) -> SizeHint {
        match self {
            HttpBody::Plain(b) => b.size_hint(),
            HttpBody::WithT(b) => b.size_hint(),
        }
    }
}

impl HttpBody {
    pub fn stream<S>(s: S) -> Self
    where
        S: Stream<Item = Result<Bytes, Infallible>> + Send + Sync + 'static,
    {
        HttpBody::Plain(PlainBody::stream(s))
    }

    pub fn full(bytes: Bytes) -> Self {
        HttpBody::Plain(PlainBody::full(bytes))
    }

    pub fn empty() -> Self {
        HttpBody::Plain(PlainBody::empty())
    }

    pub fn with_trailers<Fut>(self, fut: Fut) -> Self
    where
        Fut: std::future::Future<Output = Option<Result<http::HeaderMap, DynErr>>>
            + Send
            + Sync
            + 'static,
    {
        let boxed: BoxTrailers = Box::pin(fut);
        match self {
            HttpBody::Plain(p) => HttpBody::WithT(p.with_trailers(boxed)),
            already @ HttpBody::WithT(_) => already,
        }
    }
}

pub type HttpResponse = http::Response<HttpBody>;
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
            self.trim_end_matches('/')
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
            .and_then(|q| q.split('&').find(|p| p.starts_with(query_name)))
            .map(|p| p.split('=').nth(1).unwrap_or(""))
    }
}
