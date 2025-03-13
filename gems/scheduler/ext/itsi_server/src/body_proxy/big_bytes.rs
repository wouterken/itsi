use bytes::Bytes;
use magnus::{IntoValue, Ruby, Value};
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
