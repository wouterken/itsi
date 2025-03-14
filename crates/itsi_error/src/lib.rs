pub mod from;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, ItsiError>;

#[derive(Error, Debug)]
pub enum ItsiError {
    #[error("Invalid input {0}")]
    InvalidInput(String),
    #[error("Internal server error {0}")]
    InternalServerError(String),
    #[error("Unsupported protocol {0}")]
    UnsupportedProtocol(String),
    #[error("Argument error: {0}")]
    ArgumentError(String),
    #[error("Client Connection Closed")]
    ClientConnectionClosed,
    #[error("Jump")]
    Jump(String),
    #[error("Break")]
    Break(),
    #[error("Pass")]
    Pass(),
}
