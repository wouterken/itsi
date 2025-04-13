use super::{FromValue, MiddlewareLayer};
use crate::{
    server::http_message_types::{HttpRequest, HttpResponse, RequestExt},
    services::itsi_http_service::HttpRequestContext,
};

use async_trait::async_trait;
use http::{HeaderMap, Method, Response};
use http_body_util::{combinators::BoxBody, Empty};
use itsi_error::ItsiError;
use magnus::error::Result;
use serde::Deserialize;
use tracing::debug;

#[derive(Debug, Clone, Deserialize)]
pub struct Cors {
    pub allow_origins: Vec<String>,
    pub allow_methods: Vec<HttpMethod>,
    pub allow_headers: Vec<String>,
    pub allow_credentials: bool,
    pub expose_headers: Vec<String>,
    pub max_age: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub enum HttpMethod {
    #[serde(rename(deserialize = "GET"))]
    Get,
    #[serde(rename(deserialize = "POST"))]
    Post,
    #[serde(rename(deserialize = "PUT"))]
    Put,
    #[serde(rename(deserialize = "DELETE"))]
    Delete,
    #[serde(rename(deserialize = "OPTIONS"))]
    Options,
    #[serde(rename(deserialize = "HEAD"))]
    Head,
    #[serde(rename(deserialize = "PATCH"))]
    Patch,
}

impl HttpMethod {
    pub fn matches(&self, other: &str) -> bool {
        match self {
            HttpMethod::Get => other.eq_ignore_ascii_case("GET"),
            HttpMethod::Post => other.eq_ignore_ascii_case("POST"),
            HttpMethod::Put => other.eq_ignore_ascii_case("PUT"),
            HttpMethod::Delete => other.eq_ignore_ascii_case("DELETE"),
            HttpMethod::Options => other.eq_ignore_ascii_case("OPTIONS"),
            HttpMethod::Head => other.eq_ignore_ascii_case("HEAD"),
            HttpMethod::Patch => other.eq_ignore_ascii_case("PATCH"),
        }
    }

    pub fn to_str(&self) -> &str {
        match self {
            HttpMethod::Get => "GET",
            HttpMethod::Post => "POST",
            HttpMethod::Put => "PUT",
            HttpMethod::Delete => "DELETE",
            HttpMethod::Options => "OPTIONS",
            HttpMethod::Head => "HEAD",
            HttpMethod::Patch => "PATCH",
        }
    }
}

impl Cors {
    /// Generate the simple CORS headers (used in normal responses)
    fn cors_headers(&self, origin: &str) -> Result<HeaderMap> {
        let mut headers = HeaderMap::new();

        headers.insert("Vary", "Origin".parse().map_err(ItsiError::new)?);

        if origin.is_empty() {
            // When credentials are allowed, you cannot return "*".
            debug!(target: "middleware::cors", "Origin empty {}", origin);
            if !self.allow_credentials {
                headers.insert(
                    "Access-Control-Allow-Origin",
                    "*".parse().map_err(ItsiError::new)?,
                );
            }
            return Ok(headers);
        }

        // Only return a header if the origin is allowed.
        if self.allow_origins.iter().any(|o| o == origin || o == "*") {
            // If credentials are allowed, we must echo back the exact origin.
            let value = if self.allow_credentials {
                origin
            } else {
                // If not, and if "*" is allowed, you can still use "*".
                if self.allow_origins.iter().any(|o| o == "*") {
                    "*"
                } else {
                    origin
                }
            };
            headers.insert(
                "Access-Control-Allow-Origin",
                value.parse().map_err(ItsiError::new)?,
            );
        }

        if !self.allow_methods.is_empty() {
            headers.insert(
                "Access-Control-Allow-Methods",
                self.allow_methods
                    .iter()
                    .map(HttpMethod::to_str)
                    .collect::<Vec<&str>>()
                    .join(", ")
                    .parse()
                    .map_err(ItsiError::new)?,
            );
        }
        if !self.allow_headers.is_empty() {
            headers.insert(
                "Access-Control-Allow-Headers",
                self.allow_headers
                    .join(", ")
                    .parse()
                    .map_err(ItsiError::new)?,
            );
        }
        if self.allow_credentials {
            headers.insert(
                "Access-Control-Allow-Credentials",
                "true".parse().map_err(ItsiError::new)?,
            );
        }
        if let Some(max_age) = self.max_age {
            headers.insert(
                "Access-Control-Max-Age",
                max_age.to_string().parse().map_err(ItsiError::new)?,
            );
        }
        if !self.expose_headers.is_empty() {
            headers.insert(
                "Access-Control-Expose-Headers",
                self.expose_headers
                    .join(", ")
                    .parse()
                    .map_err(ItsiError::new)?,
            );
        }
        Ok(headers)
    }

    fn preflight_headers(
        &self,
        origin: Option<&str>,
        req_method: Option<&str>,
        req_headers: Option<&str>,
    ) -> Result<HeaderMap> {
        let mut headers = HeaderMap::new();

        headers.insert("Vary", "Origin".parse().map_err(ItsiError::new)?);

        let origin = match origin {
            Some(o) if !o.is_empty() => o,
            _ => {
                debug!(target: "middleware::cors", "Missing Origin – preflight fails");
                return Ok(headers);
            }
        };

        if !self
            .allow_origins
            .iter()
            .any(|allowed| allowed == "*" || allowed == origin)
        {
            debug!(target: "middleware::cors", "Origin not allowed");
            return Ok(headers);
        }

        let request_method = match req_method {
            Some(m) if !m.is_empty() => m,
            _ => {
                debug!(target: "middleware::cors", "Missing request method – preflight fails");
                return Ok(headers);
            }
        };

        if !self.allow_methods.iter().any(|m| m.matches(request_method)) {
            debug!(target: "middleware::cors", "Method not allowed");
            return Ok(headers);
        }

        if let Some(request_headers) = req_headers {
            let req_headers_list: Vec<&str> = request_headers
                .split(',')
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .collect();
            for header in req_headers_list {
                if !self
                    .allow_headers
                    .iter()
                    .any(|allowed| allowed.eq_ignore_ascii_case(header))
                {
                    debug!(target: "middleware::cors", "Header not allowed {}", header);
                    return Ok(headers);
                }
            }
        }

        headers.insert("Access-Control-Allow-Origin", origin.parse().unwrap());
        headers.insert(
            "Access-Control-Allow-Methods",
            self.allow_methods
                .iter()
                .map(HttpMethod::to_str)
                .collect::<Vec<&str>>()
                .join(", ")
                .parse()
                .map_err(ItsiError::new)?,
        );
        headers.insert(
            "Access-Control-Allow-Headers",
            self.allow_headers
                .join(", ")
                .parse()
                .map_err(ItsiError::new)?,
        );
        if self.allow_credentials {
            headers.insert(
                "Access-Control-Allow-Credentials",
                "true".parse().map_err(ItsiError::new)?,
            );
        }
        if let Some(max_age) = self.max_age {
            headers.insert(
                "Access-Control-Max-Age",
                max_age.to_string().parse().map_err(ItsiError::new)?,
            );
        }
        if !self.expose_headers.is_empty() {
            headers.insert(
                "Access-Control-Expose-Headers",
                self.expose_headers
                    .join(", ")
                    .parse()
                    .map_err(ItsiError::new)?,
            );
        }

        Ok(headers)
    }
}

#[async_trait]
impl MiddlewareLayer for Cors {
    // For OPTIONS (preflight) requests we:
    // 1. Extract Origin, Access-Control-Request-Method, and Access-Control-Request-Headers.
    // 2. Validate them using our hardened preflight_headers function.
    // 3. If validations pass (i.e. headers is non-empty), return a 204 response with those headers.
    // Otherwise, the absence of headers indicates the request doesn’t meet the CORS policy.
    async fn before(
        &self,
        req: HttpRequest,
        context: &mut HttpRequestContext,
    ) -> Result<either::Either<HttpRequest, HttpResponse>> {
        let origin = req.header("Origin");
        debug!(target: "middleware::cors", "Origin: {:?}", origin);
        if req.method() == Method::OPTIONS {
            let ac_request_method = req.header("Access-Control-Request-Method");
            let ac_request_headers = req.header("Access-Control-Request-Headers");
            let headers = self.preflight_headers(origin, ac_request_method, ac_request_headers)?;
            debug!(target: "middleware::cors", "Preflight Headers: {:?}", headers);
            let mut response_builder = Response::builder().status(204);
            *response_builder.headers_mut().unwrap() = headers;
            let response = response_builder
                .body(BoxBody::new(Empty::new()))
                .map_err(ItsiError::new)?;
            return Ok(either::Either::Right(response));
        }
        context.set_origin(origin.map(|s| s.to_string()));
        Ok(either::Either::Left(req))
    }

    async fn after(
        &self,
        mut resp: HttpResponse,
        context: &mut HttpRequestContext,
    ) -> HttpResponse {
        if let Some(Some(origin)) = context.origin.get() {
            debug!(target: "middleware::cors", "fetching cors headers for origin {}", origin);
            if let Ok(cors_headers) = self.cors_headers(origin) {
                debug!(target: "middleware::cors", "Cors Headers: {:?}", cors_headers);
                for (key, value) in cors_headers.iter() {
                    resp.headers_mut().insert(key.clone(), value.clone());
                }
            }
        }
        resp
    }
}
impl FromValue for Cors {}
