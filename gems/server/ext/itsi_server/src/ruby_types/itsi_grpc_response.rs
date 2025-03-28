use derive_more::Debug;
use http::request::Parts;
use tokio::sync::mpsc::Sender;

use crate::server::byte_frame::ByteFrame;

#[derive(Debug, Clone)]
#[magnus::wrap(class = "Itsi::GrpcResponse", free_immediately, size)]
pub struct ItsiGrpcResponse {
    pub parts: Parts,
    #[debug(skip)]
    pub sender: Sender<ByteFrame>,
}

impl ItsiGrpcResponse {
    pub fn new(parts: Parts, sender: Sender<ByteFrame>) -> Self {
        Self { parts, sender }
    }
}
