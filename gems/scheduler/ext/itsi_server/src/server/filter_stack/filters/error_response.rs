use crate::server::{
    static_file_cache::StaticFileCache,
    types::{HttpRequest, HttpResponse, RequestExt},
};
use bytes::Bytes;
use http::Response;
use http_body_util::{combinators::BoxBody, Full};
use serde::Deserialize;
use std::{
    path::{Path, PathBuf},
    sync::LazyLock,
    time::Duration,
};

static ROOT_STATIC_FILE_CACHE: LazyLock<StaticFileCache> = LazyLock::new(|| {
    StaticFileCache::new(
        Path::new("./"),
        4096,
        1024 * 1024 * 10,
        Duration::from_secs(1),
    )
});

#[derive(Debug, Clone, Deserialize)]
/// Filters can each have a customizable error response.
/// They can:
/// * Return Plain-text
/// * Return HTML
/// * Return JSON
pub struct ErrorResponse {
    code: u16,
    plaintext: Option<String>,
    html: Option<PathBuf>,
    json: Option<serde_json::Value>,
    default: ErrorFormat,
}

#[derive(Debug, Clone, Deserialize, Default)]
enum ErrorFormat {
    #[default]
    #[serde(rename(deserialize = "plaintext"))]
    Plaintext,
    #[serde(rename(deserialize = "html"))]
    Html,
    #[serde(rename(deserialize = "json"))]
    Json,
}

impl Default for ErrorResponse {
    fn default() -> Self {
        ErrorResponse {
            code: 500,
            plaintext: Some("Error".to_owned()),
            html: None,
            json: None,
            default: ErrorFormat::Plaintext,
        }
    }
}

impl ErrorResponse {
    pub(crate) async fn to_http_response(&self, request: &HttpRequest) -> HttpResponse {
        let accept = request.accept();
        let body = match accept {
            Some(accept) if accept.contains("text/plain") => BoxBody::new(Full::new(Bytes::from(
                self.plaintext.clone().unwrap_or_else(|| "Error".to_owned()),
            ))),
            Some(accept) if accept.contains("text/html") => {
                if let Some(path) = &self.html {
                    if let Some(file_response) =
                        ROOT_STATIC_FILE_CACHE.serve_static_file(path).await
                    {
                        file_response
                    } else {
                        BoxBody::new(Full::new(Bytes::from("Error")))
                    }
                } else {
                    BoxBody::new(Full::new(Bytes::from("Error")))
                }
            }
            Some(accept) if accept.contains("application/json") => {
                BoxBody::new(Full::new(Bytes::from(
                    self.json
                        .as_ref()
                        .map(|json| json.to_string())
                        .unwrap_or_else(|| "Error".to_owned()),
                )))
            }
            _ => match self.default {
                ErrorFormat::Plaintext => BoxBody::new(Full::new(Bytes::from(
                    self.plaintext.clone().unwrap_or_else(|| "Error".to_owned()),
                ))),
                ErrorFormat::Html => {
                    if let Some(path) = &self.html {
                        if let Some(file_response) =
                            ROOT_STATIC_FILE_CACHE.serve_static_file(path).await
                        {
                            file_response
                        } else {
                            BoxBody::new(Full::new(Bytes::from("Error")))
                        }
                    } else {
                        BoxBody::new(Full::new(Bytes::from("Error")))
                    }
                }
                ErrorFormat::Json => BoxBody::new(Full::new(Bytes::from(
                    self.json
                        .as_ref()
                        .map(|json| json.to_string())
                        .unwrap_or_else(|| "Error".to_owned()),
                ))),
            },
        };

        Response::builder().status(self.code).body(body).unwrap()
    }
}
