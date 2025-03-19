use crate::ruby_types::itsi_http_request::ItsiHttpRequest;
use itsi_rb_helpers::HeapVal;
use std::sync::Arc;

#[derive(Debug)]
pub enum RequestJob {
    ProcessRequest(ItsiHttpRequest, Arc<HeapVal>),
    Shutdown,
}
