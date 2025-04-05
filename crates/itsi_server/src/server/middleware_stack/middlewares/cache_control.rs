use crate::{
    server::http_message_types::HttpResponse, services::itsi_http_service::HttpRequestContext,
};

use super::{FromValue, MiddlewareLayer};
use async_trait::async_trait;
use http::{HeaderName, HeaderValue};
use magnus::error::Result;
use serde::Deserialize;
use std::{collections::HashMap, sync::OnceLock};

#[derive(Debug, Clone, Deserialize)]
pub struct CacheControl {
    #[serde(default)]
    pub max_age: Option<u64>,
    #[serde(default)]
    pub s_max_age: Option<u64>,
    #[serde(default)]
    pub stale_while_revalidate: Option<u64>,
    #[serde(default)]
    pub stale_if_error: Option<u64>,
    #[serde(default)]
    pub public: bool,
    #[serde(default)]
    pub private: bool,
    #[serde(default)]
    pub no_cache: bool,
    #[serde(default)]
    pub no_store: bool,
    #[serde(default)]
    pub must_revalidate: bool,
    #[serde(default)]
    pub proxy_revalidate: bool,
    #[serde(default)]
    pub immutable: bool,
    #[serde(default)]
    pub vary: Vec<String>,
    #[serde(default)]
    pub additional_headers: HashMap<String, String>,
    #[serde(skip_deserializing)]
    pub cache_control_str: OnceLock<String>,
}

#[async_trait]
impl MiddlewareLayer for CacheControl {
    async fn initialize(&self) -> Result<()> {
        let mut directives = Vec::new();

        if self.public && !self.private {
            directives.push("public".to_owned());
        } else if self.private && !self.public {
            directives.push("private".to_owned());
        }
        if self.no_cache {
            directives.push("no-cache".to_owned());
        }
        if self.no_store {
            directives.push("no-store".to_owned());
        }
        if self.must_revalidate {
            directives.push("must-revalidate".to_owned());
        }
        if self.proxy_revalidate {
            directives.push("proxy-revalidate".to_owned());
        }
        if self.immutable {
            directives.push("immutable".to_owned());
        }

        // Add age parameters
        if let Some(max_age) = self.max_age {
            directives.push(format!("max-age={}", max_age));
        }

        if let Some(s_max_age) = self.s_max_age {
            directives.push(format!("s-maxage={}", s_max_age));
        }

        if let Some(stale_while_revalidate) = self.stale_while_revalidate {
            directives.push(format!("stale-while-revalidate={}", stale_while_revalidate));
        }

        if let Some(stale_if_error) = self.stale_if_error {
            directives.push(format!("stale-if-error={}", stale_if_error));
        }

        // Set the Cache-Control header if we have directives
        if !directives.is_empty() {
            let cache_control_value = directives.join(", ");
            self.cache_control_str.set(cache_control_value).unwrap();
        }

        Ok(())
    }

    async fn after(&self, mut resp: HttpResponse, _: &mut HttpRequestContext) -> HttpResponse {
        // Skip for statuses where caching doesn't make sense
        let status = resp.status().as_u16();
        if matches!(status, 401 | 403 | 500..=599) {
            return resp;
        }

        // Set the Cache-Control header if we have directives
        if let Some(cache_control_value) = self.cache_control_str.get() {
            if let Ok(value) = HeaderValue::from_str(cache_control_value) {
                resp.headers_mut().insert("Cache-Control", value);
            }
        }

        // Set Expires header based on max-age if present
        if let Some(max_age) = self.max_age {
            // Set the Expires header based on max-age
            // Use a helper to format the HTTP date correctly
            let expires = chrono::Utc::now() + chrono::Duration::seconds(max_age as i64);
            let expires_str = expires.format("%a, %d %b %Y %H:%M:%S GMT").to_string();
            if let Ok(value) = HeaderValue::from_str(&expires_str) {
                resp.headers_mut().insert("Expires", value);
            }
        }

        // Set Vary header
        if !self.vary.is_empty() {
            let vary_value = self.vary.join(", ");
            if let Ok(value) = HeaderValue::from_str(&vary_value) {
                resp.headers_mut().insert("Vary", value);
            }
        }

        // Set additional custom headers
        for (name, value) in &self.additional_headers {
            if let Ok(header_value) = HeaderValue::from_str(value) {
                if let Ok(header_name) = name.parse::<HeaderName>() {
                    resp.headers_mut().insert(header_name, header_value);
                }
            }
        }

        resp
    }
}

impl FromValue for CacheControl {}
