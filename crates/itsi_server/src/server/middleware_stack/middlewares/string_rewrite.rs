use std::sync::OnceLock;

use serde::Deserialize;

use crate::server::{itsi_service::RequestContext, types::HttpRequest};

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

fn parse_template(template: &str) -> Vec<Segment> {
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
    pub fn rewrite(&self, req: &HttpRequest, context: &RequestContext) -> String {
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
                Segment::Literal(text) => result.push_str(text),
                Segment::Placeholder(placeholder) => {
                    let replacement = match placeholder.as_str() {
                        "method" => req.method().as_str().to_string(),
                        "path" => req.uri().path().to_string(),
                        "host" => req.uri().host().unwrap_or("localhost").to_string(),
                        "query" => {
                            let query = req.uri().query().unwrap_or("").to_string();
                            if query.is_empty() {
                                query
                            } else {
                                format!("?{}", query)
                            }
                        }
                        "port" => req
                            .uri()
                            .port()
                            .map(|p| p.to_string())
                            .unwrap_or_else(|| "80".to_string()),
                        other => {
                            // Try using the context's matching regex if available.
                            if let Some(caps) = &captures {
                                if let Some(m) = caps.name(other) {
                                    m.as_str().to_string()
                                } else {
                                    // Fallback: leave the placeholder as is.
                                    format!("{{{}}}", other)
                                }
                            } else {
                                format!("{{{}}}", other)
                            }
                        }
                    };
                    result.push_str(&replacement);
                }
            }
        }

        result
    }
}
