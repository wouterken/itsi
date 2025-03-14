use crate::ItsiError;
use std::ffi::NulError;

pub static CLIENT_CONNECTION_CLOSED: &str = "Client disconnected";

impl From<httparse::Error> for ItsiError {
    fn from(err: httparse::Error) -> Self {
        ItsiError::ArgumentError(err.to_string())
    }
}

impl From<nix::errno::Errno> for ItsiError {
    fn from(err: nix::errno::Errno) -> Self {
        ItsiError::ArgumentError(err.to_string())
    }
}

impl From<std::io::Error> for ItsiError {
    fn from(err: std::io::Error) -> Self {
        ItsiError::ArgumentError(err.to_string())
    }
}

impl From<rcgen::Error> for ItsiError {
    fn from(err: rcgen::Error) -> Self {
        ItsiError::ArgumentError(err.to_string())
    }
}

impl From<NulError> for ItsiError {
    fn from(err: NulError) -> Self {
        ItsiError::ArgumentError(err.to_string())
    }
}

impl From<magnus::Error> for ItsiError {
    fn from(err: magnus::Error) -> Self {
        match err.error_type() {
            magnus::error::ErrorType::Jump(tag) => ItsiError::Jump(tag.to_string()),
            magnus::error::ErrorType::Error(_exception_class, cow) => {
                ItsiError::ArgumentError(cow.to_string())
            }
            magnus::error::ErrorType::Exception(exception) => {
                ItsiError::ArgumentError(exception.to_string())
            }
        }
    }
}

impl From<ItsiError> for magnus::Error {
    fn from(err: ItsiError) -> Self {
        match err {
            ItsiError::InvalidInput(msg) => magnus::Error::new(magnus::exception::arg_error(), msg),
            ItsiError::InternalServerError(msg) => {
                magnus::Error::new(magnus::exception::exception(), msg)
            }
            ItsiError::UnsupportedProtocol(msg) => {
                magnus::Error::new(magnus::exception::arg_error(), msg)
            }
            ItsiError::ArgumentError(msg) => {
                magnus::Error::new(magnus::exception::arg_error(), msg)
            }
            ItsiError::Jump(msg) => magnus::Error::new(magnus::exception::local_jump_error(), msg),
            ItsiError::Break() => magnus::Error::new(magnus::exception::interrupt(), "Break"),
            ItsiError::ClientConnectionClosed => {
                magnus::Error::new(magnus::exception::eof_error(), CLIENT_CONNECTION_CLOSED)
            }
            ItsiError::Pass() => magnus::Error::new(magnus::exception::interrupt(), "Pass"),
        }
    }
}
