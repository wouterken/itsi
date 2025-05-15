use bytes::Bytes;
use http::header::CONTENT_TYPE;
use http::Response;
use serde::{Deserialize, Deserializer};
use std::path::PathBuf;
use tracing::warn;

use crate::server::http_message_types::{HttpBody, HttpResponse, ResponseFormat};
use crate::services::static_file_server::ROOT_STATIC_FILE_SERVER;
mod default_responses;

#[derive(Debug, Clone, Deserialize)]
pub enum ContentSource {
    #[serde(rename(deserialize = "inline"))]
    Inline(String),
    #[serde(rename(deserialize = "file"))]
    File(PathBuf),
    #[serde(rename(deserialize = "static"))]
    #[serde(skip_deserializing)]
    Static(Bytes),
}

#[derive(Debug, Clone, Deserialize, Default)]
pub enum DefaultFormat {
    #[serde(rename(deserialize = "plaintext"))]
    Plaintext,
    #[default]
    #[serde(rename(deserialize = "html"))]
    Html,
    #[serde(rename(deserialize = "json"))]
    Json,
}

#[derive(Debug, Clone)]
pub struct ErrorResponse {
    pub code: u16,
    pub plaintext: Option<ContentSource>,
    pub html: Option<ContentSource>,
    pub json: Option<ContentSource>,
    pub default: DefaultFormat, // must match one of the provided fields
}

impl<'de> Deserialize<'de> for ErrorResponse {
    fn deserialize<D>(deserializer: D) -> Result<ErrorResponse, D::Error>
    where
        D: Deserializer<'de>,
    {
        let def = ErrorResponseDef::deserialize(deserializer)?;
        Ok(def.into())
    }
}

/// An untagged enum to support two input formats:
/// - A detailed struct with all fields.
/// - A string with the name of a default error response.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ErrorResponseDef {
    Detailed {
        code: u16,
        plaintext: Option<ContentSource>,
        html: Option<ContentSource>,
        json: Option<ContentSource>,
        default: DefaultFormat,
    },
    Named(String),
}

impl From<ErrorResponseDef> for ErrorResponse {
    fn from(def: ErrorResponseDef) -> Self {
        match def {
            ErrorResponseDef::Detailed {
                code,
                plaintext,
                html,
                json,
                default,
            } => ErrorResponse {
                code,
                plaintext,
                html,
                json,
                default,
            },
            ErrorResponseDef::Named(name) => match name.as_str() {
                "internal_server_error" => ErrorResponse::internal_server_error(),
                "not_found" => ErrorResponse::not_found(),
                "unauthorized" => ErrorResponse::unauthorized(),
                "forbidden" => ErrorResponse::forbidden(),
                "payload_too_large" => ErrorResponse::payload_too_large(),
                "too_many_requests" => ErrorResponse::too_many_requests(),
                "bad_gateway" => ErrorResponse::bad_gateway(),
                "service_unavailable" => ErrorResponse::service_unavailable(),
                "gateway_timeout" => ErrorResponse::gateway_timeout(),
                _ => {
                    warn!(
                        "Unknown error response name: {}. Using internal server error.",
                        name
                    );
                    ErrorResponse::internal_server_error()
                }
            },
        }
    }
}

impl ErrorResponse {
    pub(crate) async fn to_http_response(&self, accept: ResponseFormat) -> HttpResponse {
        let mut resp = Response::builder().status(self.code);
        let response = match accept {
            ResponseFormat::TEXT => {
                resp = resp.header(CONTENT_TYPE, "text/plain");
                resp.body(Self::get_response_body(self.code, &self.plaintext, accept).await)
            }
            ResponseFormat::HTML => {
                resp = resp.header(CONTENT_TYPE, "text/html");
                resp.body(Self::get_response_body(self.code, &self.html, accept).await)
            }
            ResponseFormat::JSON => {
                resp = resp.header(CONTENT_TYPE, "application/json");
                resp.body(Self::get_response_body(self.code, &self.json, accept).await)
            }
            ResponseFormat::UNKNOWN => match self.default {
                DefaultFormat::Plaintext => {
                    resp = resp.header(CONTENT_TYPE, "text/plain");
                    resp.body(Self::get_response_body(self.code, &self.plaintext, accept).await)
                }
                DefaultFormat::Html => {
                    resp = resp.header(CONTENT_TYPE, "text/html");
                    resp.body(Self::get_response_body(self.code, &self.html, accept).await)
                }
                DefaultFormat::Json => {
                    resp = resp.header(CONTENT_TYPE, "application/json");
                    resp.body(Self::get_response_body(self.code, &self.json, accept).await)
                }
            },
        };
        response.unwrap()
    }

    async fn get_response_body(
        code: u16,
        source: &Option<ContentSource>,
        accept: ResponseFormat,
    ) -> HttpBody {
        match source {
            Some(ContentSource::Inline(text)) => {
                return HttpBody::full(Bytes::from(text.clone()));
            }
            Some(ContentSource::Static(text)) => {
                return HttpBody::full(text.clone());
            }
            Some(ContentSource::File(path)) => {
                // Convert the PathBuf to a &str (assumes valid UTF-8).
                if let Some(path_str) = path.to_str() {
                    let response = ROOT_STATIC_FILE_SERVER
                        .serve_single(path_str, accept, &[])
                        .await;
                    if response.status().is_success() {
                        return response.into_body();
                    }
                }
            }
            None => {}
        }
        ErrorResponse::fallback_body_for(code, accept)
    }
}
