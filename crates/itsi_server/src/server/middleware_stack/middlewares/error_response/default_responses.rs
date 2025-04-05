use std::convert::Infallible;

use bytes::Bytes;
use http_body_util::{combinators::BoxBody, Full};

use crate::server::http_message_types::ResponseFormat;

use super::{ContentSource, DefaultFormat, ErrorResponse};

impl DefaultFormat {
    pub fn response_for_code(&self, code: u16) -> ContentSource {
        match self {
            DefaultFormat::Plaintext => match code {
                500 => ContentSource::Inline("500 Internal Error".to_owned()),
                404 => ContentSource::Inline("404 Not Found".to_owned()),
                401 => ContentSource::Inline("401 Unauthorized".to_owned()),
                403 => ContentSource::Inline("403 Forbidden".to_owned()),
                413 => ContentSource::Inline("413 Payload Too Large".to_owned()),
                429 => ContentSource::Inline("429 Too Many Requests".to_owned()),
                502 => ContentSource::Inline("502 Bad Gateway".to_owned()),
                503 => ContentSource::Inline("503 Service Unavailable".to_owned()),
                504 => ContentSource::Inline("504 Gateway Timeout".to_owned()),
                _ => ContentSource::Inline("Unexpected Error".to_owned()),
            },
            DefaultFormat::Html => match code {
                500 => ContentSource::Inline(
                    include_str!("../../../../default_responses/html/500.html").to_owned(),
                ),
                404 => ContentSource::Inline(
                    include_str!("../../../../default_responses/html/404.html").to_owned(),
                ),
                401 => ContentSource::Inline(
                    include_str!("../../../../default_responses/html/401.html").to_owned(),
                ),
                403 => ContentSource::Inline(
                    include_str!("../../../../default_responses/html/403.html").to_owned(),
                ),
                413 => ContentSource::Inline(
                    include_str!("../../../../default_responses/html/413.html").to_owned(),
                ),
                429 => ContentSource::Inline(
                    include_str!("../../../../default_responses/html/429.html").to_owned(),
                ),
                502 => ContentSource::Inline(
                    include_str!("../../../../default_responses/html/502.html").to_owned(),
                ),
                503 => ContentSource::Inline(
                    include_str!("../../../../default_responses/html/503.html").to_owned(),
                ),
                504 => ContentSource::Inline(
                    include_str!("../../../../default_responses/html/504.html").to_owned(),
                ),
                _ => ContentSource::Inline("Unexpected Error".to_owned()),
            },
            DefaultFormat::Json => match code {
                500 => ContentSource::Inline(
                    include_str!("../../../../default_responses/json/500.json").to_owned(),
                ),
                404 => ContentSource::Inline(
                    include_str!("../../../../default_responses/json/404.json").to_owned(),
                ),
                401 => ContentSource::Inline(
                    include_str!("../../../../default_responses/json/401.json").to_owned(),
                ),
                403 => ContentSource::Inline(
                    include_str!("../../../../default_responses/json/403.json").to_owned(),
                ),
                413 => ContentSource::Inline(
                    include_str!("../../../../default_responses/json/413.json").to_owned(),
                ),
                429 => ContentSource::Inline(
                    include_str!("../../../../default_responses/json/429.json").to_owned(),
                ),
                502 => ContentSource::Inline(
                    include_str!("../../../../default_responses/json/502.json").to_owned(),
                ),
                503 => ContentSource::Inline(
                    include_str!("../../../../default_responses/json/503.json").to_owned(),
                ),
                504 => ContentSource::Inline(
                    include_str!("../../../../default_responses/json/504.json").to_owned(),
                ),
                _ => ContentSource::Inline("Unexpected Error".to_owned()),
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
