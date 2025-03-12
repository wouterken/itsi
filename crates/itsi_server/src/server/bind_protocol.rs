use itsi_error::ItsiError;
use std::str::FromStr;

#[derive(Debug, Default, Clone)]
pub enum BindProtocol {
    #[default]
    Https,
    Http,
    Unix,
    Unixs,
}

impl FromStr for BindProtocol {
    type Err = ItsiError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "http" => Ok(BindProtocol::Http),
            "https" => Ok(BindProtocol::Https),
            "unix" => Ok(BindProtocol::Unix),
            "tls" => Ok(BindProtocol::Unixs),
            _ => Err(ItsiError::UnsupportedProtocol(s.to_string())),
        }
    }
}

impl std::fmt::Display for BindProtocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            BindProtocol::Https => "https",
            BindProtocol::Http => "http",
            BindProtocol::Unix => "unix",
            BindProtocol::Unixs => "tls",
        };
        write!(f, "{}", s)
    }
}
