use std::convert::Infallible;

use bytes::Bytes;
use http::{Request, Response};
use http_body_util::combinators::BoxBody;
use hyper::body::Incoming;

pub type HttpResponse = Response<BoxBody<Bytes, Infallible>>;
pub type HttpRequest = Request<Incoming>;

pub trait RequestExt {
    fn content_type(&self) -> Option<&str>;
    fn accept(&self) -> Option<&str>;
    fn header(&self, header_name: &str) -> &str;
    fn query_param(&self, query_name: &str) -> &str;
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

    fn header(&self, header_name: &str) -> &str {
        self.headers()
            .get(header_name)
            .map(|hv| hv.to_str().unwrap_or(""))
            .unwrap_or("")
    }

    fn query_param(&self, query_name: &str) -> &str {
        self.uri()
            .query()
            .and_then(|query| query.split('&').find(|param| param.starts_with(query_name)))
            .map(|param| param.split('=').nth(1).unwrap_or(""))
            .unwrap_or("")
    }
}
