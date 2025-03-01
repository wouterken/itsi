use itsi_error::ItsiError;
use std::str::FromStr;

#[derive(Debug, Default, Clone)]
pub enum TransferProtocol {
    #[default]
    Https,
    Http,
    Unix,
}

impl FromStr for TransferProtocol {
    type Err = ItsiError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "http" => Ok(TransferProtocol::Http),
            "https" => Ok(TransferProtocol::Https),
            "unix" => Ok(TransferProtocol::Unix),
            _ => Err(ItsiError::UnsupportedProtocol(s.to_string())),
        }
    }
}
