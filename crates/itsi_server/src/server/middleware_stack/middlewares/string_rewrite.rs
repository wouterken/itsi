use serde::Deserialize;
use std::sync::OnceLock;

use crate::{
    server::http_message_types::{HttpRequest, HttpResponse},
    services::itsi_http_service::HttpRequestContext,
};

#[derive(Debug, Clone, Deserialize)]
#[serde(transparent)]
pub struct StringRewrite {
    pub template_string: String,
    #[serde(default)]
    pub segments: OnceLock<Vec<Segment>>,
}

#[derive(Debug, Clone, Deserialize)]
pub enum Segment {
    Literal(String),
    Placeholder(String),
}

pub fn parse_template(template: &str) -> Vec<Segment> {
    let mut segments = Vec::new();
    let mut last_index = 0;
    while let Some(start_index) = template[last_index..].find('{') {
        let start_index = last_index + start_index;
        // Add the literal text before the placeholder.
        if start_index > last_index {
            segments.push(Segment::Literal(
                template[last_index..start_index].to_string(),
            ));
        }
        // Find the corresponding closing brace.
        if let Some(end_index) = template[start_index..].find('}') {
            let end_index = start_index + end_index;
            let placeholder = &template[start_index + 1..end_index];
            segments.push(Segment::Placeholder(placeholder.to_string()));
            last_index = end_index + 1;
        } else {
            // No closing brace found; treat the rest as literal.
            segments.push(Segment::Literal(template[start_index..].to_string()));
            break;
        }
    }
    if last_index < template.len() {
        segments.push(Segment::Literal(template[last_index..].to_string()));
    }
    segments
}

impl StringRewrite {
    /// Apply a single modifier of the form `op:arg` (or for replace `op:from,to`)
    #[inline]
    fn apply_modifier(s: &mut String, mod_str: &str) {
        if let Some((op, arg)) = mod_str.split_once(':') {
            match op {
                "strip_prefix" => {
                    if s.starts_with(arg) {
                        let _ = s.drain(..arg.len());
                    }
                }
                "strip_suffix" => {
                    if s.ends_with(arg) {
                        let len = s.len();
                        let start = len.saturating_sub(arg.len());
                        let _ = s.drain(start..);
                    }
                }
                "replace" => {
                    if let Some((from, to)) = arg.split_once(',') {
                        if s.contains(from) {
                            *s = s.replace(from, to);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    pub fn rewrite_request(&self, req: &HttpRequest, context: &HttpRequestContext) -> String {
        let segments = self
            .segments
            .get_or_init(|| parse_template(&self.template_string));
        let captures = context
            .matching_pattern
            .as_ref()
            .and_then(|re| re.captures(req.uri().path()));

        let mut result = String::with_capacity(self.template_string.len());

        for segment in segments {
            match segment {
                Segment::Literal(text) => {
                    result.push_str(text);
                }
                Segment::Placeholder(raw) => {
                    // split into key and optional modifier
                    let mut parts = raw.split('|');
                    let key = parts.next().unwrap();
                    let modifiers = parts; // zero o

                    // 1) lookup the raw replacement
                    let mut replacement = match key {
                        "request_id" => context.short_request_id(),
                        "request_id_full" => context.request_id(),
                        "method" => req.method().as_str().to_string(),
                        "path" => req.uri().path().to_string(),
                        "addr" => context.addr.to_owned(),
                        "host" => req.uri().host().unwrap_or("localhost").to_string(),
                        "path_and_query" => req
                            .uri()
                            .path_and_query()
                            .map(|pq| pq.to_string())
                            .unwrap_or_default(),
                        "query" => {
                            let q = req.uri().query().unwrap_or("");
                            if q.is_empty() {
                                "".to_string()
                            } else {
                                format!("?{}", q)
                            }
                        }
                        "port" => req
                            .uri()
                            .port()
                            .map(|p| p.to_string())
                            .unwrap_or_else(|| "80".to_string()),
                        "start_time" => {
                            if let Some(ts) = context.start_time() {
                                ts.format("%Y-%m-%d:%H:%M:%S:%3f").to_string()
                            } else {
                                "N/A".to_string()
                            }
                        }
                        other => {
                            // headers first
                            if let Some(hv) = req.headers().get(other) {
                                hv.to_str().unwrap_or("").to_string()
                            }
                            // then any regex‐capture
                            else if let Some(caps) = &captures {
                                caps.name(other)
                                    .map(|m| m.as_str().to_string())
                                    .unwrap_or_else(|| format!("{{{}}}", other))
                            }
                            // fallback: leave placeholder intact
                            else {
                                format!("{{{}}}", other)
                            }
                        }
                    };

                    for m in modifiers {
                        Self::apply_modifier(&mut replacement, m);
                    }

                    result.push_str(&replacement);
                }
            }
        }

        result
    }

    pub fn rewrite_response(&self, resp: &HttpResponse, context: &HttpRequestContext) -> String {
        let segments = self
            .segments
            .get_or_init(|| parse_template(&self.template_string));

        let mut result = String::with_capacity(self.template_string.len());
        for segment in segments {
            match segment {
                Segment::Literal(text) => {
                    result.push_str(text);
                }
                Segment::Placeholder(raw) => {
                    let mut parts = raw.split('|');
                    let key = parts.next().unwrap();
                    let modifiers = parts; // zero o

                    let mut replacement = match key {
                        "request_id" => context.short_request_id(),
                        "request_id_full" => context.request_id(),
                        "status" => resp.status().as_str().to_string(),
                        "addr" => context.addr.to_owned(),
                        "response_time" => {
                            let dur = context.get_response_time();
                            let micros = dur.as_micros();
                            if micros < 1_000 {
                                format!("{}µs", micros)
                            } else {
                                let ms = dur.as_secs_f64() * 1_000.0;
                                format!("{:.3}ms", ms)
                            }
                        }
                        other => {
                            if let Some(hv) = resp.headers().get(other) {
                                hv.to_str().unwrap_or("").to_string()
                            } else {
                                format!("{{{}}}", other)
                            }
                        }
                    };

                    for m in modifiers {
                        Self::apply_modifier(&mut replacement, m);
                    }

                    result.push_str(&replacement);
                }
            }
        }

        result
    }
}
