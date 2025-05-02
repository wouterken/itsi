use super::{ContentSource, DefaultFormat, ErrorResponse};
use crate::server::http_message_types::ResponseFormat;
use bytes::Bytes;
use http_body_util::{combinators::BoxBody, Full};
use std::convert::Infallible;

impl DefaultFormat {
    pub fn response_for_code(&self, code: u16) -> ContentSource {
        match self {
            DefaultFormat::Plaintext => match code {
                500 => ContentSource::Static(Full::new("500 Internal Error".into())),
                404 => ContentSource::Static(Full::new("404 Not Found".into())),
                401 => ContentSource::Static(Full::new("401 Unauthorized".into())),
                403 => ContentSource::Static(Full::new("403 Forbidden".into())),
                413 => ContentSource::Static(Full::new("413 Payload Too Large".into())),
                429 => ContentSource::Static(Full::new("429 Too Many Requests".into())),
                502 => ContentSource::Static(Full::new("502 Bad Gateway".into())),
                503 => ContentSource::Static(Full::new("503 Service Unavailable".into())),
                504 => ContentSource::Static(Full::new("504 Gateway Timeout".into())),
                _ => ContentSource::Static(Full::new("Unexpected Error".into())),
            },
            DefaultFormat::Html => match code {
                500 => ContentSource::Static(Full::new(
                    include_str!("../../../../default_responses/html/500.html").into(),
                )),
                404 => ContentSource::Static(Full::new(
                    include_str!("../../../../default_responses/html/404.html").into(),
                )),
                401 => ContentSource::Static(Full::new(
                    include_str!("../../../../default_responses/html/401.html").into(),
                )),
                403 => ContentSource::Static(Full::new(
                    include_str!("../../../../default_responses/html/403.html").into(),
                )),
                413 => ContentSource::Static(Full::new(
                    include_str!("../../../../default_responses/html/413.html").into(),
                )),
                429 => ContentSource::Static(Full::new(
                    include_str!("../../../../default_responses/html/429.html").into(),
                )),
                502 => ContentSource::Static(Full::new(
                    include_str!("../../../../default_responses/html/502.html").into(),
                )),
                503 => ContentSource::Static(Full::new(
                    include_str!("../../../../default_responses/html/503.html").into(),
                )),
                504 => ContentSource::Static(Full::new(
                    include_str!("../../../../default_responses/html/504.html").into(),
                )),
                _ => ContentSource::Static(Full::new(
                    include_str!("../../../../default_responses/html/500.html").into(),
                )),
            },
            DefaultFormat::Json => match code {
                500 => ContentSource::Static(Full::new(
                    include_str!("../../../../default_responses/json/500.json").into(),
                )),
                404 => ContentSource::Static(Full::new(
                    include_str!("../../../../default_responses/json/404.json").into(),
                )),
                401 => ContentSource::Static(Full::new(
                    include_str!("../../../../default_responses/json/401.json").into(),
                )),
                403 => ContentSource::Static(Full::new(
                    include_str!("../../../../default_responses/json/403.json").into(),
                )),
                413 => ContentSource::Static(Full::new(
                    include_str!("../../../../default_responses/json/413.json").into(),
                )),
                429 => ContentSource::Static(Full::new(
                    include_str!("../../../../default_responses/json/429.json").into(),
                )),
                502 => ContentSource::Static(Full::new(
                    include_str!("../../../../default_responses/json/502.json").into(),
                )),
                503 => ContentSource::Static(Full::new(
                    include_str!("../../../../default_responses/json/503.json").into(),
                )),
                504 => ContentSource::Static(Full::new(
                    include_str!("../../../../default_responses/json/504.json").into(),
                )),
                _ => ContentSource::Static(Full::new("Unexpected Error".into())),
            },
        }
    }
}
impl ErrorResponse {
    pub fn fallback_body_for(code: u16, accept: ResponseFormat) -> BoxBody<Bytes, Infallible> {
        let source = match accept {
            ResponseFormat::TEXT => DefaultFormat::Plaintext.response_for_code(code),
            ResponseFormat::HTML => DefaultFormat::Html.response_for_code(code),
            ResponseFormat::JSON => DefaultFormat::Json.response_for_code(code),
            ResponseFormat::UNKNOWN => ContentSource::Inline("Unexpected Error".to_owned()),
        };
        match source {
            ContentSource::Inline(bytes) => BoxBody::new(Full::new(Bytes::from(bytes))),
            ContentSource::Static(bytes) => BoxBody::new(bytes),
            ContentSource::File(_) => BoxBody::new(Full::new(Bytes::from("Unexpected error"))),
        }
    }
    pub fn internal_server_error() -> Self {
        ErrorResponse {
            code: 500,
            plaintext: Some(DefaultFormat::Plaintext.response_for_code(500)),
            html: Some(DefaultFormat::Html.response_for_code(500)),
            json: Some(DefaultFormat::Json.response_for_code(500)),
            default: DefaultFormat::Html,
        }
    }

    pub fn not_found() -> Self {
        ErrorResponse {
            code: 404,
            plaintext: Some(DefaultFormat::Plaintext.response_for_code(404)),
            html: Some(DefaultFormat::Html.response_for_code(404)),
            json: Some(DefaultFormat::Json.response_for_code(404)),
            default: DefaultFormat::Html,
        }
    }

    pub fn unauthorized() -> Self {
        ErrorResponse {
            code: 401,
            plaintext: Some(DefaultFormat::Plaintext.response_for_code(401)),
            html: Some(DefaultFormat::Html.response_for_code(401)),
            json: Some(DefaultFormat::Json.response_for_code(401)),
            default: DefaultFormat::Html,
        }
    }

    pub fn forbidden() -> Self {
        ErrorResponse {
            code: 403,
            plaintext: Some(DefaultFormat::Plaintext.response_for_code(403)),
            html: Some(DefaultFormat::Html.response_for_code(403)),
            json: Some(DefaultFormat::Json.response_for_code(403)),
            default: DefaultFormat::Html,
        }
    }

    pub fn payload_too_large() -> Self {
        ErrorResponse {
            code: 413,
            plaintext: Some(DefaultFormat::Plaintext.response_for_code(413)),
            html: Some(DefaultFormat::Html.response_for_code(413)),
            json: Some(DefaultFormat::Json.response_for_code(413)),
            default: DefaultFormat::Html,
        }
    }

    pub fn too_many_requests() -> Self {
        ErrorResponse {
            code: 429,
            plaintext: Some(DefaultFormat::Plaintext.response_for_code(429)),
            html: Some(DefaultFormat::Html.response_for_code(429)),
            json: Some(DefaultFormat::Json.response_for_code(429)),
            default: DefaultFormat::Html,
        }
    }

    pub fn bad_gateway() -> Self {
        ErrorResponse {
            code: 502,
            plaintext: Some(DefaultFormat::Plaintext.response_for_code(502)),
            html: Some(DefaultFormat::Html.response_for_code(502)),
            json: Some(DefaultFormat::Json.response_for_code(502)),
            default: DefaultFormat::Html,
        }
    }

    pub fn service_unavailable() -> Self {
        ErrorResponse {
            code: 503,
            plaintext: Some(DefaultFormat::Plaintext.response_for_code(503)),
            html: Some(DefaultFormat::Html.response_for_code(503)),
            json: Some(DefaultFormat::Json.response_for_code(503)),
            default: DefaultFormat::Html,
        }
    }

    pub fn gateway_timeout() -> Self {
        ErrorResponse {
            code: 504,
            plaintext: Some(DefaultFormat::Plaintext.response_for_code(504)),
            html: Some(DefaultFormat::Html.response_for_code(504)),
            json: Some(DefaultFormat::Json.response_for_code(504)),
            default: DefaultFormat::Html,
        }
    }
}
