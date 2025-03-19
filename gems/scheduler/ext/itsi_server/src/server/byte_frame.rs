use std::ops::Deref;

use bytes::Bytes;

#[derive(Debug)]
pub enum ByteFrame {
    Data(Bytes),
    End(Bytes),
    Empty,
}

impl Deref for ByteFrame {
    type Target = Bytes;

    fn deref(&self) -> &Self::Target {
        match self {
            ByteFrame::Data(data) => data,
            ByteFrame::End(data) => data,
            ByteFrame::Empty => unreachable!(),
        }
    }
}

impl From<ByteFrame> for Bytes {
    fn from(frame: ByteFrame) -> Self {
        match frame {
            ByteFrame::Data(data) => data,
            ByteFrame::End(data) => data,
            ByteFrame::Empty => unreachable!(),
        }
    }
}
