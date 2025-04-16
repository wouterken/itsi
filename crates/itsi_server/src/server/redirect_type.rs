use http::StatusCode;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, Default)]
pub enum RedirectType {
    #[serde(rename(deserialize = "permanent"))]
    #[default]
    Permanent,
    #[serde(rename(deserialize = "temporary"))]
    Temporary,
    #[serde(rename(deserialize = "found"))]
    Found,
    #[serde(rename(deserialize = "moved_permanently"))]
    MovedPermanently,
}

impl RedirectType {
    pub fn status_code(&self) -> StatusCode {
        match self {
            RedirectType::Permanent => StatusCode::PERMANENT_REDIRECT,
            RedirectType::Temporary => StatusCode::TEMPORARY_REDIRECT,
            RedirectType::Found => StatusCode::FOUND,
            RedirectType::MovedPermanently => StatusCode::MOVED_PERMANENTLY,
        }
    }
}
