use super::FromValue;
use crate::{
    server::http_message_types::{HttpRequest, HttpResponse},
    services::itsi_http_service::HttpRequestContext,
};
use async_trait::async_trait;
use bytes::{Bytes, BytesMut};
use either::Either;
use futures::TryStreamExt;
use http::{HeaderValue, StatusCode};
use http_body_util::{combinators::BoxBody, BodyExt, Empty};
use itsi_error::ItsiError;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::{path::PathBuf, sync::OnceLock};
use tokio::sync::Mutex;
use tokio::time::{self, Duration};
use tracing::debug;

#[derive(Debug, Serialize, Deserialize)]
pub struct CspReport {
    #[serde(rename = "csp-report")]
    pub report: ReportDetails,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReportDetails {
    #[serde(rename = "document-uri")]
    pub document_uri: String,
    #[serde(rename = "referrer")]
    pub referrer: Option<String>,
    #[serde(rename = "violated-directive")]
    pub violated_directive: String,
    #[serde(rename = "original-policy")]
    pub original_policy: String,
    #[serde(rename = "blocked-uri")]
    pub blocked_uri: String,
}

#[derive(Debug, Deserialize)]
pub struct CspConfig {
    pub default_src: Vec<String>,
    pub script_src: Vec<String>,
    pub style_src: Vec<String>,
    pub report_uri: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct Csp {
    pub policy: Option<CspConfig>,
    pub reporting_enabled: bool,
    pub report_file: Option<PathBuf>,
    pub report_endpoint: String,
    pub flush_interval: f64,

    #[serde(skip)]
    pub computed_policy: OnceLock<String>,
    #[serde(skip)]
    pub pending_reports: Arc<Mutex<Vec<CspReport>>>,
    #[serde(skip)]
    pub flush_task: OnceLock<tokio::task::JoinHandle<()>>,
}

#[async_trait]
impl super::MiddlewareLayer for Csp {
    async fn initialize(&self) -> Result<(), magnus::error::Error> {
        if let Some(policy_config) = &self.policy {
            let mut parts = Vec::new();
            if !policy_config.default_src.is_empty() {
                parts.push(format!(
                    "default-src {}",
                    policy_config.default_src.join(" ")
                ));
            }
            if !policy_config.script_src.is_empty() {
                parts.push(format!("script-src {}", policy_config.script_src.join(" ")));
            }
            if !policy_config.style_src.is_empty() {
                parts.push(format!("style-src {}", policy_config.style_src.join(" ")));
            }
            if !policy_config.report_uri.is_empty() {
                parts.push(format!("report-uri {}", policy_config.report_uri.join(" ")));
            }
            let policy = parts.join("; ");
            debug!(target: "middleware::csp", "Computed CSP policy: {}", policy);
            self.computed_policy
                .set(policy)
                .map_err(|_| ItsiError::new("Failed to set computed CSP policy"))?;
        }

        if self.reporting_enabled {
            if let Some(ref report_file) = self.report_file {
                let flush_interval = self.flush_interval;
                let report_path = report_file.clone();
                let pending_reports = Arc::clone(&self.pending_reports);
                let handle = tokio::spawn(async move {
                    let mut interval = time::interval(Duration::from_secs_f64(flush_interval));
                    loop {
                        interval.tick().await;

                        let mut reports = pending_reports.lock().await;
                        if !reports.is_empty() {
                            let mut lines = String::new();
                            for report in reports.iter() {
                                if let Ok(line) = serde_json::to_string(report) {
                                    lines.push_str(&line);
                                    lines.push('\n');
                                }
                            }
                            reports.clear();

                            debug!("Flushing CSP report to file {:?}", &report_path.display());

                            use tokio::io::AsyncWriteExt;

                            match tokio::fs::OpenOptions::new()
                                .append(true)
                                .create(true)
                                .open(&report_path)
                                .await
                            {
                                Ok(mut file) => {
                                    if let Err(e) = file.write_all(lines.as_bytes()).await {
                                        eprintln!("Error writing CSP reports: {:?}", e);
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Error opening CSP report file: {:?}", e);
                                }
                            }
                        }
                    }
                });
                self.flush_task
                    .set(handle)
                    .map_err(|_| ItsiError::new("Failed to set flush task handle"))?;
            }
        }
        Ok(())
    }

    async fn before(
        &self,
        req: HttpRequest,
        _context: &mut HttpRequestContext,
    ) -> Result<Either<HttpRequest, HttpResponse>, magnus::error::Error> {
        if self.reporting_enabled && req.uri().path() == self.report_endpoint {
            debug!(target: "middleware::csp", "Received CSP report");
            let full_bytes: Result<Bytes, _> = req
                .into_body()
                .into_data_stream()
                .try_fold(BytesMut::new(), |mut acc, chunk| async move {
                    acc.extend_from_slice(&chunk);
                    Ok(acc)
                })
                .await
                .map(|b| b.freeze());

            if let Ok(body_bytes) = full_bytes {
                if let Ok(report) = serde_json::from_slice::<CspReport>(&body_bytes) {
                    debug!(target: "middleware::csp", "Report: {:?}", report);
                    let mut pending = self.pending_reports.lock().await;
                    pending.push(report);
                }
            }

            let mut resp = HttpResponse::new(BoxBody::new(Empty::new()));
            *resp.status_mut() = StatusCode::NO_CONTENT;
            return Ok(Either::Right(resp));
        }
        Ok(Either::Left(req))
    }

    async fn after(&self, resp: HttpResponse, _context: &mut HttpRequestContext) -> HttpResponse {
        if let Some(policy) = self.computed_policy.get() {
            if !resp.headers().contains_key("Content-Security-Policy") {
                debug!(target: "middleware::csp", "Adding CSP header");
                let (mut parts, body) = resp.into_parts();
                if let Ok(header_value) = HeaderValue::from_str(policy) {
                    parts
                        .headers
                        .insert("Content-Security-Policy", header_value);
                }
                return HttpResponse::from_parts(parts, body);
            } else {
                debug!(target: "middleware::csp", "CSP header already present");
            }
        }
        resp
    }
}

impl FromValue for Csp {}
