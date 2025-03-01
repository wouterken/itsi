use bytes::Bytes;
use magnus::{error::Result as MagnusResult, exception::io_error, Error};
use tokio::sync::mpsc::Sender;

#[magnus::wrap(class = "Itsi::StreamWriter", free_immediately, size)]
pub struct StreamWriter {
    sender: Sender<Bytes>,
}

impl StreamWriter {
    pub fn new(sender: Sender<Bytes>) -> Self {
        StreamWriter { sender }
    }

    pub fn write(&self, bytes: Bytes) -> MagnusResult<()> {
        self.sender
            .blocking_send(bytes)
            .map_err(|e| Error::new(io_error(), format!("{:?}", e)))?;
        Ok(())
    }
}
