use crate::ruby_types::{itsi_grpc_request::ItsiGrpcRequest, itsi_http_request::ItsiHttpRequest};
use itsi_rb_helpers::HeapValue;
use magnus::block::Proc;
use std::sync::Arc;

#[derive(Debug)]
pub enum RequestJob {
    ProcessHttpRequest(ItsiHttpRequest, Arc<HeapValue<Proc>>),
    ProcessGrpcRequest(ItsiGrpcRequest, Arc<HeapValue<Proc>>),
    Shutdown,
}
