use magnus::{
    Error,
    error::ErrorType,
    exception::{self, arg_error, exception},
};
use nix::errno::Errno;

use crate::ItsiError;
use std::{ffi::NulError, io};

pub static CLIENT_CONNECTION_CLOSED: &str = "Client disconnected";

impl From<httparse::Error> for ItsiError {
    fn from(err: httparse::Error) -> Self {
        ItsiError::ArgumentError(err.to_string())
    }
}

impl From<Errno> for ItsiError {
    fn from(err: Errno) -> Self {
        ItsiError::ArgumentError(err.to_string())
    }
}

impl From<io::Error> for ItsiError {
    fn from(err: io::Error) -> Self {
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

impl From<Error> for ItsiError {
    fn from(err: Error) -> Self {
        match err.error_type() {
            ErrorType::Jump(tag) => ItsiError::Jump(tag.to_string()),
            ErrorType::Error(_exception_class, cow) => ItsiError::ArgumentError(cow.to_string()),
            ErrorType::Exception(exception) => ItsiError::ArgumentError(exception.to_string()),
        }
    }
}

impl From<ItsiError> for Error {
    fn from(err: ItsiError) -> Self {
        match err {
            ItsiError::InvalidInput(msg) => Error::new(arg_error(), msg),
            ItsiError::InternalServerError(msg) => Error::new(exception(), msg),
            ItsiError::UnsupportedProtocol(msg) => Error::new(arg_error(), msg),
            ItsiError::ArgumentError(msg) => Error::new(arg_error(), msg),
            ItsiError::Jump(msg) => Error::new(exception::local_jump_error(), msg),
            ItsiError::Break() => Error::new(exception::interrupt(), "Break"),
            ItsiError::ClientConnectionClosed => {
                Error::new(exception::eof_error(), CLIENT_CONNECTION_CLOSED)
            }
            ItsiError::Pass() => Error::new(exception::interrupt(), "Pass"),
        }
    }
}
