pub use anyhow::Context;
use magnus::Error as MagnusError;
use magnus::{
    error::ErrorType,
    exception::{self, arg_error, standard_error},
};
use thiserror::Error;

pub static CLIENT_CONNECTION_CLOSED: &str = "Client disconnected";
pub type Result<T> = std::result::Result<T, ItsiError>;

#[derive(Error, Debug)]
pub enum ItsiError {
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Internal server error: {0}")]
    InternalServerError(String),

    #[error("Unsupported protocol: {0}")]
    UnsupportedProtocol(String),

    #[error("Argument error: {0}")]
    ArgumentError(String),

    #[error("Client Connection Closed")]
    ClientConnectionClosed,

    #[error("Internal Error")]
    InternalError(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Rcgen(#[from] rcgen::Error),

    #[error(transparent)]
    HttpParse(#[from] httparse::Error),

    #[error(transparent)]
    NixErrno(#[from] nix::errno::Errno),

    #[error(transparent)]
    Nul(#[from] std::ffi::NulError),

    #[error("Jump: {0}")]
    Jump(String),

    #[error("Break")]
    Break,

    #[error("Pass")]
    Pass,

    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),
}

impl From<magnus::Error> for ItsiError {
    fn from(err: magnus::Error) -> Self {
        match err.error_type() {
            ErrorType::Jump(tag) => ItsiError::Jump(tag.to_string()),
            ErrorType::Error(_exception_class, cow) => ItsiError::ArgumentError(cow.to_string()),
            ErrorType::Exception(exception) => ItsiError::ArgumentError(exception.to_string()),
        }
    }
}

pub trait IntoMagnusError {
    fn into_magnus_error(self) -> MagnusError;
}

impl<T: std::error::Error> IntoMagnusError for T {
    fn into_magnus_error(self) -> MagnusError {
        MagnusError::new(standard_error(), self.to_string())
    }
}

impl From<&str> for ItsiError {
    fn from(s: &str) -> Self {
        ItsiError::InternalError(s.to_owned())
    }
}

impl From<String> for ItsiError {
    fn from(s: String) -> Self {
        ItsiError::InternalError(s)
    }
}

impl From<ItsiError> for magnus::Error {
    fn from(err: ItsiError) -> Self {
        match err {
            ItsiError::InvalidInput(msg) => magnus::Error::new(arg_error(), msg),
            ItsiError::InternalServerError(msg) => magnus::Error::new(standard_error(), msg),
            ItsiError::InternalError(msg) => magnus::Error::new(standard_error(), msg),
            ItsiError::UnsupportedProtocol(msg) => magnus::Error::new(arg_error(), msg),
            ItsiError::ArgumentError(msg) => magnus::Error::new(arg_error(), msg),
            ItsiError::Jump(msg) => magnus::Error::new(exception::local_jump_error(), msg),
            ItsiError::ClientConnectionClosed => {
                magnus::Error::new(exception::eof_error(), CLIENT_CONNECTION_CLOSED)
            }
            ItsiError::Break => magnus::Error::new(exception::interrupt(), "Break"),
            ItsiError::Pass => magnus::Error::new(exception::interrupt(), "Pass"),
            ItsiError::Io(err) => err.into_magnus_error(),
            ItsiError::Rcgen(err) => err.into_magnus_error(),
            ItsiError::HttpParse(err) => err.into_magnus_error(),
            ItsiError::NixErrno(err) => err.into_magnus_error(),
            ItsiError::Nul(err) => err.into_magnus_error(),
            ItsiError::Anyhow(err) => err.into_magnus_error(),
        }
    }
}

impl ItsiError {
    pub fn new(error: impl Send + Sync + 'static + std::fmt::Display) -> Self {
        ItsiError::InternalError(format!("{}", error))
    }
}

unsafe impl Send for ItsiError {}
unsafe impl Sync for ItsiError {}
