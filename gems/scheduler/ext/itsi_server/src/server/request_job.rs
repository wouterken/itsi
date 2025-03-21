use crate::ruby_types::itsi_http_request::ItsiHttpRequest;
use itsi_rb_helpers::HeapValue;
use magnus::block::Proc;
use std::sync::Arc;

#[derive(Debug)]
pub enum RequestJob {
    ProcessRequest(ItsiHttpRequest, Arc<HeapValue<Proc>>),
    Shutdown,
}
