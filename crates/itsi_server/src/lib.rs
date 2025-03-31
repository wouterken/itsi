use magnus::{error::Result, function, method, Module, Object, Ruby};
use ruby_types::{
    itsi_body_proxy::ItsiBodyProxy, itsi_grpc_call::ItsiGrpcCall, itsi_grpc_stream::ItsiGrpcStream,
    itsi_http_request::ItsiHttpRequest, itsi_http_response::ItsiHttpResponse,
    itsi_server::ItsiServer, ITSI_BODY_PROXY, ITSI_GRPC_CALL, ITSI_GRPC_RESPONSE, ITSI_GRPC_STREAM,
    ITSI_MODULE, ITSI_REQUEST, ITSI_RESPONSE, ITSI_SERVER,
};
use server::signal::reset_signal_handlers;
use tracing::*;
pub mod env;
pub mod prelude;
pub mod ruby_types;
pub mod server;
#[magnus::init]
fn init(ruby: &Ruby) -> Result<()> {
    itsi_tracing::init();
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .ok();

    let itsi = ruby.get_inner(&ITSI_MODULE);
    itsi.define_singleton_method("log_debug", function!(log_debug, 1))?;
    itsi.define_singleton_method("log_info", function!(log_info, 1))?;
    itsi.define_singleton_method("log_warn", function!(log_warn, 1))?;
    itsi.define_singleton_method("log_error", function!(log_error, 1))?;

    let server = ruby.get_inner(&ITSI_SERVER);
    server.define_singleton_method("new", function!(ItsiServer::new, 3))?;
    server.define_singleton_method("reset_signal_handlers", function!(reset_signal_handlers, 0))?;
    server.define_method("start", method!(ItsiServer::start, 0))?;
    server.define_method("stop", method!(ItsiServer::stop, 0))?;

    let request = ruby.get_inner(&ITSI_REQUEST);
    request.define_method("path", method!(ItsiHttpRequest::path, 0))?;
    request.define_method("script_name", method!(ItsiHttpRequest::script_name, 0))?;
    request.define_method("query_string", method!(ItsiHttpRequest::query_string, 0))?;
    request.define_method("request_method", method!(ItsiHttpRequest::method, 0))?;
    request.define_method("version", method!(ItsiHttpRequest::version, 0))?;
    request.define_method("rack_protocol", method!(ItsiHttpRequest::rack_protocol, 0))?;
    request.define_method("host", method!(ItsiHttpRequest::host, 0))?;
    request.define_method("headers", method!(ItsiHttpRequest::headers, 0))?;
    request.define_method("header", method!(ItsiHttpRequest::header, 1))?;
    request.define_method("[]", method!(ItsiHttpRequest::header, 1))?;
    request.define_method("scheme", method!(ItsiHttpRequest::scheme, 0))?;
    request.define_method("remote_addr", method!(ItsiHttpRequest::remote_addr, 0))?;
    request.define_method("port", method!(ItsiHttpRequest::port, 0))?;
    request.define_method("body", method!(ItsiHttpRequest::body, 0))?;
    request.define_method("response", method!(ItsiHttpRequest::response, 0))?;
    request.define_method("json?", method!(ItsiHttpRequest::is_json, 0))?;
    request.define_method("html?", method!(ItsiHttpRequest::is_html, 0))?;

    let body_proxy = ruby.get_inner(&ITSI_BODY_PROXY);
    body_proxy.define_method("gets", method!(ItsiBodyProxy::gets, 0))?;
    body_proxy.define_method("each", method!(ItsiBodyProxy::each, 0))?;
    body_proxy.define_method("read", method!(ItsiBodyProxy::read, -1))?;
    body_proxy.define_method("close", method!(ItsiBodyProxy::close, 0))?;

    let response = ruby.get_inner(&ITSI_RESPONSE);
    response.define_method("[]=", method!(ItsiHttpResponse::add_header, 2))?;
    response.define_method("add_header", method!(ItsiHttpResponse::add_header, 2))?;
    response.define_method("add_headers", method!(ItsiHttpResponse::add_headers, 1))?;
    response.define_method("status=", method!(ItsiHttpResponse::set_status, 1))?;
    response.define_method("send_frame", method!(ItsiHttpResponse::send_frame, 1))?;
    response.define_method("<<", method!(ItsiHttpResponse::send_frame, 1))?;
    response.define_method("write", method!(ItsiHttpResponse::send_frame, 1))?;
    response.define_method("read", method!(ItsiHttpResponse::recv_frame, 0))?;
    response.define_method(
        "send_and_close",
        method!(ItsiHttpResponse::send_and_close, 1),
    )?;
    response.define_method("close_write", method!(ItsiHttpResponse::close_write, 0))?;
    response.define_method("close_read", method!(ItsiHttpResponse::close_read, 0))?;
    response.define_method("close", method!(ItsiHttpResponse::close, 0))?;
    response.define_method("hijack", method!(ItsiHttpResponse::hijack, 1))?;
    response.define_method("json?", method!(ItsiHttpResponse::is_json, 0))?;
    response.define_method("html?", method!(ItsiHttpResponse::is_html, 0))?;

    let grpc_call = ruby.get_inner(&ITSI_GRPC_CALL);
    grpc_call.define_method("service_name", method!(ItsiGrpcCall::service_name, 0))?;
    grpc_call.define_method("method_name", method!(ItsiGrpcCall::method_name, 0))?;
    grpc_call.define_method("stream", method!(ItsiGrpcCall::stream, 0))?;
    grpc_call.define_method("json?", method!(ItsiGrpcCall::is_json, 0))?;
    grpc_call.define_method("content_type", method!(ItsiGrpcCall::content_type_str, 0))?;
    grpc_call.define_method("timeout", method!(ItsiGrpcCall::timeout, 0))?;
    grpc_call.define_method("cancelled?", method!(ItsiGrpcCall::is_cancelled, 0))?;
    grpc_call.define_method("add_headers", method!(ItsiGrpcCall::add_headers, 1))?;

    grpc_call.define_method(
        "decompress_input",
        method!(ItsiGrpcCall::decompress_input, 1),
    )?;
    grpc_call.define_method("compress_output", method!(ItsiGrpcCall::compress_output, 1))?;
    grpc_call.define_method(
        "should_compress_output?",
        method!(ItsiGrpcCall::should_compress_output, 1),
    )?;

    let grpc_stream = ruby.get_inner(&ITSI_GRPC_STREAM);
    grpc_stream.define_method("reader_fileno", method!(ItsiGrpcStream::reader, 0))?;
    grpc_stream.define_method("write", method!(ItsiGrpcStream::write, 1))?;
    grpc_stream.define_method("flush", method!(ItsiGrpcStream::flush, 0))?;
    grpc_stream.define_method("send_trailers", method!(ItsiGrpcStream::send_trailers, 1))?;
    grpc_stream.define_method("close", method!(ItsiGrpcStream::close, 0))?;

    let _grpc_response = ruby.get_inner(&ITSI_GRPC_RESPONSE);

    Ok(())
}

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
