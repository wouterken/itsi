use magnus::{error::Result, function, method, value::Lazy, Module, Object, RClass, RModule, Ruby};
use request::itsi_request::ItsiRequest;
use server::itsi_server::Server;
use stream_writer::StreamWriter;

pub mod request;
pub mod response;
pub mod server;
pub mod stream_writer;

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
pub static ITSI_STREAM_WRITER: Lazy<RClass> = Lazy::new(|ruby| {
    ruby.get_inner(&ITSI_MODULE)
        .define_class("StreamWriter", ruby.class_object())
        .unwrap()
});

#[magnus::init]
fn init(ruby: &Ruby) -> Result<()> {
    itsi_tracing::init();

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

    let stream_writer = ruby.get_inner(&ITSI_STREAM_WRITER);
    stream_writer.define_method("write", method!(StreamWriter::write, 1))?;

    Ok(())
}
