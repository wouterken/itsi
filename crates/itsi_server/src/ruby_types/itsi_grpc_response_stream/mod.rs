use super::itsi_grpc_call::CompressionAlgorithm;
use crate::prelude::*;
use crate::server::http_message_types::HttpResponse;
use crate::server::size_limited_incoming::SizeLimitedIncoming;
use crate::server::{byte_frame::ByteFrame, serve_strategy::single_mode::RunningPhase};
use bytes::Bytes;
use derive_more::Debug;
use futures::stream::unfold;
use http::Version;
use http::{
    header::{HeaderName, HeaderValue},
    HeaderMap, Response,
};
use http_body_util::{combinators::BoxBody, BodyDataStream, BodyExt, Empty, Full, StreamBody};
use hyper::body::{Frame, Incoming};
use magnus::error::Result as MagnusResult;
use nix::unistd::pipe;
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::{
    collections::HashMap,
    os::fd::{AsRawFd, FromRawFd, IntoRawFd, OwnedFd},
    sync::Arc,
};
use tokio::{
    spawn,
    sync::{
        mpsc::{self, Sender},
        oneshot, watch,
    },
};
use tokio_stream::{wrappers::ReceiverStream, StreamExt};

#[derive(Debug, Clone)]
#[magnus::wrap(class = "Itsi::GrpcResponseStream", free_immediately, size)]
pub struct ItsiGrpcResponseStream {
    pub inner: Arc<Mutex<ItsiGrpcResponseStreamInner>>,
    pub cancelled: Arc<AtomicBool>,
}

#[derive(Debug)]
pub struct ItsiGrpcResponseStreamInner {
    pub incoming_reader: Option<OwnedFd>,
    pub buf: Vec<u8>,
    pub response_sender: Sender<ByteFrame>,
    pub response: Option<HttpResponse>,
    pub response_headers: HeaderMap,
    trailer_tx: oneshot::Sender<HeaderMap>,
    trailer_rx: Option<oneshot::Receiver<HeaderMap>>,
}

impl ItsiGrpcResponseStreamInner {
    pub fn reader(&mut self) -> MagnusResult<i32> {
        Ok(self.incoming_reader.take().unwrap().into_raw_fd())
    }

    pub fn write(&mut self, bytes: Bytes) -> MagnusResult<()> {
        self.response_sender
            .blocking_send(ByteFrame::Data(bytes))
            .map_err(|err| {
                magnus::Error::new(
                    magnus::exception::io_error(),
                    format!("Trying to write to closed stream: {:?}", err),
                )
            })?;
        Ok(())
    }

    pub fn flush(&mut self) -> MagnusResult<()> {
        Ok(())
    }

    pub fn send_trailers(&mut self, trailers: HashMap<String, String>) -> MagnusResult<()> {
        let mut header_map = HeaderMap::new();
        for (key, value) in trailers {
            if let (Ok(hn), Ok(hv)) = (key.parse::<HeaderName>(), value.parse::<HeaderValue>()) {
                header_map.insert(hn, hv);
            }
        }
        let trailer_tx = std::mem::replace(&mut self.trailer_tx, oneshot::channel().0);
        trailer_tx.send(header_map).map_err(|err| {
            magnus::Error::new(
                magnus::exception::standard_error(),
                format!("Error sending trailers {:?}", err),
            )
        })?;
        Ok(())
    }

    pub fn close(&mut self) -> MagnusResult<()> {
        self.response_sender.blocking_send(ByteFrame::Empty).ok();
        Ok(())
    }

    pub fn add_headers(&mut self, headers: HashMap<Bytes, Vec<Bytes>>) -> MagnusResult<()> {
        for (name, values) in headers {
            let header_name = HeaderName::from_bytes(&name).map_err(|e| {
                itsi_error::ItsiError::InvalidInput(format!(
                    "Invalid header name {:?}: {:?}",
                    name, e
                ))
            })?;
            for value in values {
                let header_value = unsafe { HeaderValue::from_maybe_shared_unchecked(value) };
                self.response_headers.insert(&header_name, header_value);
            }
        }

        Ok(())
    }
}

impl ItsiGrpcResponseStream {
    pub async fn new(
        compression_out: CompressionAlgorithm,
        response_sender: Sender<ByteFrame>,
        mut body: BodyDataStream<SizeLimitedIncoming<Incoming>>,
    ) -> Self {
        let (trailer_tx, trailer_rx) = oneshot::channel::<HeaderMap>();
        let (pipe_read, pipe_write) = pipe().unwrap();

        nix::fcntl::fcntl(
            pipe_read.as_raw_fd(),
            nix::fcntl::FcntlArg::F_SETFL(nix::fcntl::OFlag::O_NONBLOCK),
        )
        .unwrap();

        nix::fcntl::fcntl(
            pipe_write.as_raw_fd(),
            nix::fcntl::FcntlArg::F_SETFL(nix::fcntl::OFlag::O_NONBLOCK),
        )
        .unwrap();

        let pipe_raw_fd = pipe_write.into_raw_fd();

        let cancelled = Arc::new(AtomicBool::new(false));
        let cancelled_clone = cancelled.clone();
        spawn(async move {
            use std::io::Write;
            let mut write_end = unsafe { std::fs::File::from_raw_fd(pipe_raw_fd) };
            while let Some(Ok(body)) = body.next().await {
                write_end.write_all(&body).unwrap();
            }
            cancelled_clone.store(true, Ordering::SeqCst);
        });

        let mut response_headers = HeaderMap::new();

        match compression_out {
            CompressionAlgorithm::None => (),
            CompressionAlgorithm::Deflate => {
                response_headers.insert("grpc-encoding", "deflate".parse().unwrap());
            }
            CompressionAlgorithm::Gzip => {
                response_headers.insert("grpc-encoding", "gzip".parse().unwrap());
            }
        }
        ItsiGrpcResponseStream {
            inner: Arc::new(Mutex::new(ItsiGrpcResponseStreamInner {
                buf: Vec::new(),
                response_headers,
                incoming_reader: Some(pipe_read),
                response_sender,
                response: Some(Response::new(BoxBody::new(Empty::new()))),
                trailer_tx,
                trailer_rx: Some(trailer_rx),
            })),
            cancelled,
        }
    }

    pub fn reader(&self) -> MagnusResult<i32> {
        self.inner.lock().reader()
    }

    pub fn write(&self, bytes: Bytes) -> MagnusResult<()> {
        self.inner.lock().write(bytes)
    }

    pub fn flush(&self) -> MagnusResult<()> {
        self.inner.lock().flush()
    }

    pub fn is_cancelled(&self) -> MagnusResult<bool> {
        Ok(self.cancelled.load(Ordering::SeqCst))
    }

    pub fn send_trailers(&self, trailers: HashMap<String, String>) -> MagnusResult<()> {
        self.inner.lock().send_trailers(trailers)
    }

    pub fn close(&self) -> MagnusResult<()> {
        self.inner.lock().close()
    }

    pub fn add_headers(&self, headers: HashMap<Bytes, Vec<Bytes>>) -> MagnusResult<()> {
        self.inner.lock().add_headers(headers)
    }

    pub async fn build_response(
        &self,
        first_frame: ByteFrame,
        receiver: mpsc::Receiver<ByteFrame>,
        shutdown_rx: watch::Receiver<RunningPhase>,
    ) -> HttpResponse {
        let mut response = self.inner.lock().response.take().unwrap();
        let rx = self.inner.lock().trailer_rx.take().unwrap();
        *response.version_mut() = Version::HTTP_2;
        *response.headers_mut() = self.inner.lock().response_headers.clone();
        *response.body_mut() = if matches!(first_frame, ByteFrame::Empty) {
            BoxBody::new(Empty::new())
        } else if matches!(first_frame, ByteFrame::End(_)) {
            BoxBody::new(Full::new(first_frame.into()))
        } else {
            let initial_frame = tokio_stream::once(Ok(Frame::data(Bytes::from(first_frame))));
            let frame_stream = unfold(
                (ReceiverStream::new(receiver), shutdown_rx),
                |(mut receiver, mut shutdown_rx)| async move {
                    if let RunningPhase::ShutdownPending = *shutdown_rx.borrow() {
                        return None;
                    }
                    loop {
                        tokio::select! {
                            maybe_bytes = receiver.next() => {
                              match maybe_bytes {
                                Some(ByteFrame::Data(bytes)) | Some(ByteFrame::End(bytes)) => {
                                  return Some((Ok(Frame::data(bytes)), (receiver, shutdown_rx)));
                                }
                                _ => {
                                  return None;
                                }
                              }
                            },
                            _ = shutdown_rx.changed() => {
                                match *shutdown_rx.borrow() {
                                    RunningPhase::ShutdownPending => {
                                        debug!("Disconnecting streaming client.");
                                        return None;
                                    },
                                    _ => continue,
                                }
                            }
                        }
                    }
                },
            );

            let combined_stream = initial_frame.chain(frame_stream);
            BoxBody::new(StreamBody::new(combined_stream))
        }
        .with_trailers(async move {
            match rx.await {
                Ok(trailers) => Some(Ok(trailers)),
                Err(_err) => None,
            }
        })
        .boxed();
        response
    }

    pub fn internal_server_error(&self, message: String) {
        error!(message);
    }
}
