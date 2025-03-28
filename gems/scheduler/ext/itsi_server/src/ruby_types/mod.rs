use magnus::{value::Lazy, Module, RClass, RModule};

pub mod itsi_body_proxy;
pub mod itsi_grpc_request;
pub mod itsi_grpc_response;
pub mod itsi_grpc_stream;
pub mod itsi_http_request;
pub mod itsi_http_response;
pub mod itsi_server;

pub static ITSI_MODULE: Lazy<RModule> = Lazy::new(|ruby| ruby.define_module("Itsi").unwrap());
pub static ITSI_SERVER: Lazy<RClass> = Lazy::new(|ruby| {
    ruby.get_inner(&ITSI_MODULE)
        .define_class("Server", ruby.class_object())
        .unwrap()
});

pub static ITSI_SERVER_CONFIG: Lazy<RModule> =
    Lazy::new(|ruby| ruby.get_inner(&ITSI_SERVER).const_get("Config").unwrap());

pub static ITSI_REQUEST: Lazy<RClass> = Lazy::new(|ruby| {
    ruby.get_inner(&ITSI_MODULE)
        .define_class("HttpRequest", ruby.class_object())
        .unwrap()
});

pub static ITSI_RESPONSE: Lazy<RClass> = Lazy::new(|ruby| {
    ruby.get_inner(&ITSI_MODULE)
        .define_class("HttpResponse", ruby.class_object())
        .unwrap()
});

pub static ITSI_BODY_PROXY: Lazy<RClass> = Lazy::new(|ruby| {
    ruby.get_inner(&ITSI_MODULE)
        .define_class("BodyProxy", ruby.class_object())
        .unwrap()
});

pub static ITSI_GRPC_REQUEST: Lazy<RClass> = Lazy::new(|ruby| {
    ruby.get_inner(&ITSI_MODULE)
        .define_class("GrpcRequest", ruby.class_object())
        .unwrap()
});

pub static ITSI_GRPC_STREAM: Lazy<RClass> = Lazy::new(|ruby| {
    ruby.get_inner(&ITSI_MODULE)
        .define_class("GrpcStream", ruby.class_object())
        .unwrap()
});

pub static ITSI_GRPC_RESPONSE: Lazy<RClass> = Lazy::new(|ruby| {
    ruby.get_inner(&ITSI_MODULE)
        .define_class("GrpcResponse", ruby.class_object())
        .unwrap()
});
