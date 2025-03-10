use itsi_error::ItsiError;
use std::str::FromStr;

#[derive(Debug, Default, Clone)]
pub enum BindProtocol {
    #[default]
    Https,
    Http,
    Unix,
}

impl FromStr for BindProtocol {
    type Err = ItsiError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "http" => Ok(BindProtocol::Http),
            "https" => Ok(BindProtocol::Https),
            "unix" => Ok(BindProtocol::Unix),
            _ => Err(ItsiError::UnsupportedProtocol(s.to_string())),
        }
    }
}
