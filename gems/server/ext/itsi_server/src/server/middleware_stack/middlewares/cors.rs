use async_trait::async_trait;
use magnus::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::server::{
    itsi_service::RequestContext,
    types::{HttpRequest, HttpResponse, RequestExt},
};

use super::{FromValue, MiddlewareLayer};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cors {
    pub allowed_origins: Vec<String>,
    pub allowed_methods: Vec<String>,
    pub allowed_headers: Vec<String>,
    pub exposed_headers: Vec<String>,
    pub allow_credentials: bool,
    pub max_age: Option<u64>,
}

impl Cors {
    /// Computes the CORS headers based on the incoming request's Origin.
    fn cors_headers(&self, origin: Option<&str>) -> HashMap<String, String> {
        let mut headers = HashMap::new();

        // Determine the allowed origin.
        // If the allowed_origins contains "*" or the origin is explicitly allowed, use it.
        if let Some(req_origin) = origin {
            if self.allowed_origins.contains(&"*".to_string())
                || self.allowed_origins.contains(&req_origin.to_string())
            {
                headers.insert(
                    "Access-Control-Allow-Origin".to_string(),
                    req_origin.to_string(),
                );
            }
        } else {
            // If no Origin header is present, you might default to "*" (or leave it out).
            headers.insert("Access-Control-Allow-Origin".to_string(), "*".to_string());
        }

        if !self.allowed_methods.is_empty() {
            headers.insert(
                "Access-Control-Allow-Methods".to_string(),
                self.allowed_methods.join(", "),
            );
        }
        if !self.allowed_headers.is_empty() {
            headers.insert(
                "Access-Control-Allow-Headers".to_string(),
                self.allowed_headers.join(", "),
            );
        }
        if self.allow_credentials {
            headers.insert(
                "Access-Control-Allow-Credentials".to_string(),
                "true".to_string(),
            );
        }
        if let Some(max_age) = self.max_age {
            headers.insert("Access-Control-Max-Age".to_string(), max_age.to_string());
        }
        if !self.exposed_headers.is_empty() {
            headers.insert(
                "Access-Control-Expose-Headers".to_string(),
                self.exposed_headers.join(", "),
            );
        }
        headers
    }
}

#[async_trait]
impl MiddlewareLayer for Cors {
    // The before hook is called early in the processing of a request.
    // Here we check for an OPTIONS preflight request and return a 200 response with CORS headers.
    async fn before(
        &self,
        req: HttpRequest,
        _context: &mut RequestContext,
    ) -> Result<either::Either<HttpRequest, HttpResponse>> {
        todo!();
        // if req.method().eq_ignore_ascii_case("OPTIONS") {
        //     // Extract the Origin header, if available.
        //     let origin = {
        //         let origin_val = req.header("Origin");
        //         if origin_val.is_empty() {
        //             None
        //         } else {
        //             Some(origin_val.as_str())
        //         }
        //     };
        //     let cors_headers = self.cors_headers(origin);
        //     let mut response_builder = HttpResponse::builder().status(200);
        //     for (key, value) in cors_headers.iter() {
        //         response_builder = response_builder.header(key, value);
        //     }
        //     let response = response_builder.body("".into()).unwrap();
        //     return Ok(either::Either::Right(response));
        // }
        Ok(either::Either::Left(req))
    }

    // The after hook allows modifying the response before it is sent.
    // Here we inject the CORS headers (if an Origin header was present in the request).
    async fn after(&self, resp: HttpResponse, _context: &mut RequestContext) -> HttpResponse {
        todo!();
        // let origin = {
        //     let origin_val = req.header("Origin");
        //     if origin_val.is_empty() {
        //         None
        //     } else {
        //         Some(origin_val.as_str())
        //     }
        // };
        // if origin.is_some() {
        //     let cors_headers = self.cors_headers(origin);
        //     for (key, value) in cors_headers.iter() {
        //         // Here we assume that res.headers_mut() returns a mutable header map.
        //         resp.headers_mut()
        //             .insert(key.parse().unwrap(), value.parse().unwrap());
        //     }
        // }
        resp
    }
}

impl FromValue for Cors {}
