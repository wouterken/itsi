pub mod big_bytes;
use big_bytes::BigBytes;
use bytes::Bytes;
use futures::executor::block_on;
use http_body_util::{BodyDataStream, BodyExt};
use hyper::body::Incoming;
use magnus::{error::Result as MagnusResult, scan_args, IntoValue, RString, Ruby, Value};
use parking_lot::Mutex;
use std::sync::{
    atomic::{self, AtomicBool},
    Arc,
};
use tokio_stream::StreamExt;

#[magnus::wrap(class = "Itsi::BodyProxy", free_immediately, size)]
#[derive(Debug, Clone)]
pub struct ItsiBodyProxy {
    pub incoming: Arc<Mutex<BodyDataStream<Incoming>>>,
    pub closed: Arc<AtomicBool>,
    pub buf: Arc<Mutex<Vec<u8>>>,
}

pub enum ItsiBody {
    Buffered(BigBytes),
    Stream(ItsiBodyProxy),
}

impl ItsiBody {
    pub fn into_value(&self) -> Option<Value> {
        match self {
            ItsiBody::Buffered(bytes) => bytes.as_value(),
            ItsiBody::Stream(proxy) => Some(proxy.clone().into_value()),
        }
    }
}
impl ItsiBodyProxy {
    pub fn new(incoming: Incoming) -> Self {
        ItsiBodyProxy {
            incoming: Arc::new(Mutex::new(incoming.into_data_stream())),
            closed: Arc::new(AtomicBool::new(false)),
            buf: Arc::new(Mutex::new(vec![])),
        }
    }
    /// Read up to the next line-break OR EOF
    pub fn gets(&self) -> MagnusResult<Option<Bytes>> {
        self.verify_open()?;
        let mut stream = self.incoming.lock();
        let mut buf = self.buf.lock();
        while !buf.contains(&b'\n') {
            if let Some(chunk) = block_on(stream.next()) {
                let chunk = chunk.map_err(|err| {
                    magnus::Error::new(
                        magnus::exception::exception(),
                        format!("Error reading body {:?}", err),
                    )
                })?;
                buf.extend_from_slice(&chunk);
            } else {
                break;
            }
        }
        if let Some(pos) = buf.iter().position(|&x| x == b'\n') {
            let line = buf.drain(..=pos).collect::<Vec<u8>>();
            Ok(Some(line.into()))
        } else if !buf.is_empty() {
            let line = buf.drain(..).collect::<Vec<u8>>();
            Ok(Some(line.into()))
        } else {
            Ok(None)
        }
    }

    pub fn read(&self, args: &[Value]) -> MagnusResult<Option<RString>> {
        self.verify_open()?;
        let scanned =
            scan_args::scan_args::<(), (Option<usize>, Option<RString>), (), (), (), ()>(args)?;
        let (length, mut buffer) = scanned.optional;
        let mut stream = self.incoming.lock();
        let mut buf = self.buf.lock();

        while length.is_none_or(|target_length| buf.len() < target_length) {
            if let Some(chunk) = block_on(stream.next()) {
                let chunk = chunk.map_err(|err| {
                    magnus::Error::new(
                        magnus::exception::exception(),
                        format!("Error reading body {:?}", err),
                    )
                })?;
                buf.extend_from_slice(&chunk);
            } else if length.is_some() {
                return Ok(None);
            } else {
                break;
            }
        }
        let output_string = buffer.take().unwrap_or(RString::buf_new(buf.len()));
        output_string.cat(buf.clone());
        buf.clear();
        Ok(Some(output_string))
    }

    /// Equivalent to calling gets and yielding it, until we reach EOF
    pub fn each(ruby: &Ruby, rbself: &Self) -> MagnusResult<()> {
        let proc = ruby.block_proc()?;
        while let Some(str) = rbself.gets()? {
            proc.call::<_, Value>((str,))?;
        }
        Ok(())
    }

    fn verify_open(&self) -> MagnusResult<()> {
        if self.closed.load(atomic::Ordering::SeqCst) {
            return Err(magnus::Error::new(
                magnus::exception::exception(),
                "Body stream is closed",
            ));
        }
        Ok(())
    }
    pub fn close(&self) {
        self.closed.store(true, atomic::Ordering::SeqCst);
    }
}
