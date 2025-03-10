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
    pub fn into_value(&self) -> Value {
        match self {
            ItsiBody::Buffered(bytes) => bytes.as_value(),
            ItsiBody::Stream(proxy) => proxy.clone().into_value(),
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

use std::io::{Result as IoResult, Write};
use std::path::PathBuf;
use tempfile::NamedTempFile;

const THRESHOLD: usize = 1024 * 1024; // 1 MB

/// An container that holds data in memory if it’s small, or in a temporary file on disk if it exceeds THRESHOLD.
/// Used for providing Rack input data.
pub enum BigBytes {
    InMemory(Vec<u8>),
    OnDisk(NamedTempFile),
}

/// The result type for reading the contents of a `BigBytes` value.
pub enum BigBytesReadResult {
    /// When the data is stored in memory, returns the cached bytes.
    InMemory(Vec<u8>),
    /// When the data is stored on disk, returns the path to the temporary file.
    OnDisk(PathBuf),
}

impl BigBytes {
    /// Creates a new, empty BigBytes instance (initially in memory).
    pub fn new() -> Self {
        BigBytes::InMemory(Vec::new())
    }

    /// Reads the entire contents that have been written.
    ///
    /// - If stored in memory, returns a clone of the bytes.
    /// - If stored on disk, returns the file path of the temporary file.
    pub fn read(&self) -> IoResult<BigBytesReadResult> {
        match self {
            BigBytes::InMemory(vec) => Ok(BigBytesReadResult::InMemory(vec.clone())),
            BigBytes::OnDisk(temp_file) => {
                // Flush to be safe, then return the file path.
                temp_file.as_file().sync_all()?;
                Ok(BigBytesReadResult::OnDisk(temp_file.path().to_path_buf()))
            }
        }
    }

    pub fn as_value(&self) -> Value {
        match self {
            BigBytes::InMemory(bytes) => {
                let bytes = Bytes::from(bytes.to_owned());
                bytes.into_value()
            }
            BigBytes::OnDisk(path) => {
                let ruby = Ruby::get().unwrap();
                let rarray = ruby.ary_new();
                rarray.push(path.path().to_str().unwrap().into_value()).ok();
                rarray.into_value()
            }
        }
    }
}

impl Drop for BigBytes {
    fn drop(&mut self) {
        match self {
            BigBytes::InMemory(_) => {}
            BigBytes::OnDisk(path) => {
                let _ = std::fs::remove_file(path);
            }
        }
    }
}

impl Default for BigBytes {
    fn default() -> Self {
        Self::new()
    }
}

impl Write for BigBytes {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        match self {
            BigBytes::InMemory(vec) => {
                // Check if writing the new bytes would exceed the threshold.
                if vec.len() + buf.len() > THRESHOLD {
                    let mut tmp = NamedTempFile::new()?;
                    tmp.write_all(vec)?;
                    tmp.write_all(buf)?;
                    *self = BigBytes::OnDisk(tmp);
                    Ok(buf.len())
                } else {
                    vec.extend_from_slice(buf);
                    Ok(buf.len())
                }
            }
            BigBytes::OnDisk(tmp_file) => tmp_file.write(buf),
        }
    }

    fn flush(&mut self) -> IoResult<()> {
        match self {
            BigBytes::InMemory(_) => Ok(()),
            BigBytes::OnDisk(tmp_file) => tmp_file.flush(),
        }
    }
}
