use std::sync::LazyLock;

use crate::server::middleware_stack::ErrorResponse;

pub static TIMEOUT_RESPONSE: LazyLock<ErrorResponse> =
    LazyLock::new(ErrorResponse::gateway_timeout);

pub static NOT_FOUND_RESPONSE: LazyLock<ErrorResponse> = LazyLock::new(ErrorResponse::not_found);

pub static INTERNAL_SERVER_ERROR_RESPONSE: LazyLock<ErrorResponse> =
    LazyLock::new(ErrorResponse::internal_server_error);
