use magnus::{error::Result, function, method, value::Lazy, Module, Object, RClass, RModule, Ruby};
use request::itsi_request::ItsiRequest;
use response::itsi_response::ItsiResponse;
use server::itsi_server::Server;
use tracing::*;

pub mod request;
pub mod response;
pub mod server;

pub static ITSI_MODULE: Lazy<RModule> = Lazy::new(|ruby| ruby.define_module("Itsi").unwrap());
pub static ITSI_SERVER: Lazy<RClass> = Lazy::new(|ruby| {
    ruby.get_inner(&ITSI_MODULE)
        .define_class("Server", ruby.class_object())
        .unwrap()
});
pub static ITSI_REQUEST: Lazy<RClass> = Lazy::new(|ruby| {
    ruby.get_inner(&ITSI_MODULE)
        .define_class("Request", ruby.class_object())
        .unwrap()
});

pub static ITSI_RESPONSE: Lazy<RClass> = Lazy::new(|ruby| {
    ruby.get_inner(&ITSI_MODULE)
        .define_class("Response", ruby.class_object())
        .unwrap()
});

pub static ITSI_STREAM_WRITER: Lazy<RClass> = Lazy::new(|ruby| {
    ruby.get_inner(&ITSI_MODULE)
        .define_class("StreamWriter", ruby.class_object())
        .unwrap()
});

pub fn log_debug(msg: String) {
    debug!(msg);
}
pub fn log_info(msg: String) {
    info!(msg);
}
pub fn log_warn(msg: String) {
    warn!(msg);
}
pub fn log_error(msg: String) {
    error!(msg);
}

#[magnus::init]
fn init(ruby: &Ruby) -> Result<()> {
    itsi_tracing::init();

    let itsi = ruby.get_inner(&ITSI_MODULE);
    itsi.define_singleton_method("log_debug", function!(log_debug, 1))?;
    itsi.define_singleton_method("log_info", function!(log_info, 1))?;
    itsi.define_singleton_method("log_warn", function!(log_warn, 1))?;
    itsi.define_singleton_method("log_error", function!(log_error, 1))?;

    let server = ruby.get_inner(&ITSI_SERVER);
    server.define_singleton_method("new", function!(Server::new, -1))?;
    server.define_method("start", method!(Server::start, 0))?;

    let request = ruby.get_inner(&ITSI_REQUEST);
    request.define_method("path", method!(ItsiRequest::path, 0))?;
    request.define_method("script_name", method!(ItsiRequest::script_name, 0))?;
    request.define_method("query_string", method!(ItsiRequest::query_string, 0))?;
    request.define_method("method", method!(ItsiRequest::method, 0))?;
    request.define_method("version", method!(ItsiRequest::version, 0))?;
    request.define_method("rack_protocol", method!(ItsiRequest::rack_protocol, 0))?;
    request.define_method("host", method!(ItsiRequest::host, 0))?;
    request.define_method("headers", method!(ItsiRequest::headers, 0))?;
    request.define_method("scheme", method!(ItsiRequest::scheme, 0))?;
    request.define_method("remote_addr", method!(ItsiRequest::remote_addr, 0))?;
    request.define_method("port", method!(ItsiRequest::port, 0))?;
    request.define_method("body", method!(ItsiRequest::body, 0))?;
    request.define_method("response", method!(ItsiRequest::response, 0))?;

    let response = ruby.get_inner(&ITSI_RESPONSE);
    response.define_method("add_header", method!(ItsiResponse::add_header, 2))?;
    response.define_method("status=", method!(ItsiResponse::set_status, 1))?;
    response.define_method("send_frame", method!(ItsiResponse::send_frame, 1))?;
    response.define_method("send_and_close", method!(ItsiResponse::send_and_close, 1))?;
    response.define_method("close_write", method!(ItsiResponse::close_write, 0))?;
    response.define_method("close_read", method!(ItsiResponse::close_read, 0))?;
    response.define_method("close", method!(ItsiResponse::close, 0))?;
    response.define_method("hijack", method!(ItsiResponse::hijack, 1))?;

    Ok(())
}
