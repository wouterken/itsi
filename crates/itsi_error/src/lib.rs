use thiserror::Error;

pub type Result<T> = std::result::Result<T, ItsiError>;

#[derive(Error, Debug)]
pub enum ItsiError {
    #[error("Invalid input")]
    InvalidInput(String),
    #[error("Internal server error")]
    InternalServerError,
    #[error("Unsupported protocol")]
    UnsupportedProtocol(String),
    #[error("Argument error")]
    ArgumentError(String),
    #[error("Jump")]
    Jump(String),
    #[error("Break")]
    Break(),
}

impl From<ItsiError> for magnus::Error {
    fn from(err: ItsiError) -> Self {
        magnus::Error::new(magnus::exception::runtime_error(), err.to_string())
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
