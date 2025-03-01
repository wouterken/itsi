use std::{convert::Infallible, str::FromStr, sync::Arc};

use bytes::Bytes;
use http::{request::Parts, HeaderMap, HeaderName, HeaderValue, Response, StatusCode};
use http_body_util::combinators::BoxBody;
use itsi_tracing::error;
#[magnus::wrap(class = "Itsi::Response", free_immediately, size)]
#[derive(Debug)]
pub struct ItsiResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: String,
    pub parts: Arc<Parts>,
}

impl ItsiResponse {}
impl From<ItsiResponse> for Response<BoxBody<Bytes, Infallible>> {
    fn from(value: ItsiResponse) -> Self {
        let status_code =
            StatusCode::from_u16(value.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        let mut headers = HeaderMap::new();
        value
            .headers
            .into_iter()
            .for_each(|(header_name, header_value)| {
                let header_name = HeaderName::from_str(&header_name);
                let header_value = HeaderValue::from_str(&header_value);
                match (header_name, header_value) {
                    (Ok(header_name), Ok(header_value)) => {
                        headers.insert(header_name, header_value);
                    }
                    (v1, v2) => {
                        error!("Invalid header name or value {:?}, {:?}", v1, v2);
                    }
                }
            });
        let mut response = Response::new(BoxBody::new(value.body));
        *response.status_mut() = status_code;
        *response.headers_mut() = headers;
        response
    }
}
