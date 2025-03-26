use crate::server::static_file_server::ROOT_STATIC_FILE_SERVER;
use crate::server::types::RequestExt;
use crate::server::{
    itsi_service::RequestContext,
    types::{HttpRequest, HttpResponse},
};

use bytes::Bytes;
use either::Either;
use http::Response;
use http_body_util::{combinators::BoxBody, Full};
use itsi_error::ItsiError;
use serde::Deserialize;
use std::path::{Path, PathBuf};

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
                    let path = path.to_str().unwrap();
                    let response = ROOT_STATIC_FILE_SERVER.serve_single(path).await;

                    if response.status().is_success() {
                        response.into_body()
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
                        let path = path.to_str().unwrap();
                        let response = ROOT_STATIC_FILE_SERVER.serve_single(path).await;

                        if response.status().is_success() {
                            response.into_body()
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

    pub async fn before(
        &self,
        req: HttpRequest,
        _context: &mut RequestContext,
    ) -> Result<Either<HttpRequest, HttpResponse>, ItsiError> {
        if let Some(path) = req.uri().path().strip_prefix("/error/") {
            let path = Path::new(path);
            if path.exists() {
                let path = path.to_str().unwrap();
                let response = ROOT_STATIC_FILE_SERVER.serve_single(path).await;
                return Ok(Either::Right(response));
            }
        }
        Ok(Either::Left(req))
    }
}
