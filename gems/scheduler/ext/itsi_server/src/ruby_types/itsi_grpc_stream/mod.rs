use std::{collections::HashMap, sync::Arc};

use crate::server::{
    byte_frame::ByteFrame, serve_strategy::single_mode::RunningPhase, types::HttpResponse,
};
use bytes::Bytes;
use derive_more::Debug;
use futures::{executor::block_on, stream::unfold};
use http::{
    header::{HeaderName, HeaderValue, CONTENT_TYPE},
    HeaderMap, Response,
};
use http_body_util::{combinators::BoxBody, BodyDataStream, BodyExt, Empty, Full, StreamBody};
use hyper::body::{Frame, Incoming};
use magnus::error::Result as MagnusResult;
use parking_lot::Mutex;
use tokio::sync::{
    mpsc::{Receiver, Sender},
    oneshot, watch,
};
use tokio_stream::{wrappers::ReceiverStream, StreamExt};
use tracing::{error, info, warn};

#[derive(Debug, Clone)]
#[magnus::wrap(class = "Itsi::GrpcStream", free_immediately, size)]
pub struct ItsiGrpcStream {
    pub inner: Arc<Mutex<ItsiGrpcStreamInner>>,
}

#[derive(Debug)]
pub struct ItsiGrpcStreamInner {
    pub body: BodyDataStream<Incoming>,
    pub buf: Vec<u8>,
    pub response_sender: Sender<ByteFrame>,
    pub response: Option<HttpResponse>,
    trailer_tx: oneshot::Sender<HeaderMap>,
    trailer_rx: Option<oneshot::Receiver<HeaderMap>>,
}

impl ItsiGrpcStreamInner {
    pub fn read(&mut self, bytes: usize) -> MagnusResult<Bytes> {
        let stream = &mut self.body;
        let buf = &mut self.buf;
        let mut result = Vec::with_capacity(bytes);

        info!("Entering read with {:?}. Current buf is {:?}", bytes, buf);

        // First, use any data already in the buffer
        if !buf.is_empty() {
            let remaining = bytes.min(buf.len());
            result.extend_from_slice(&buf[..remaining]);
            buf.drain(..remaining);
        }

        while result.len() < bytes {
            if let Some(chunk) = block_on(stream.next()) {
                let chunk = chunk.map_err(|err| {
                    magnus::Error::new(
                        magnus::exception::exception(),
                        format!("Error reading body {:?}", err),
                    )
                })?;
                let remaining = bytes - result.len();
                if chunk.len() > remaining {
                    result.extend_from_slice(&chunk[..remaining]);
                    buf.extend_from_slice(&chunk[remaining..]);
                } else {
                    result.extend_from_slice(&chunk);
                }
            } else {
                break;
            }
        }

        Ok(result.into())
    }

    pub fn write(&mut self, bytes: Bytes) -> MagnusResult<()> {
        self.response_sender
            .blocking_send(ByteFrame::Data(bytes))
            .map_err(|err| {
                magnus::Error::new(
                    magnus::exception::exception(),
                    format!("Error writing body {:?}", err),
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
                magnus::exception::exception(),
                format!("Error sending trailers {:?}", err),
            )
        })?;
        self.response_sender
            .blocking_send(ByteFrame::Empty)
            .map_err(|err| {
                magnus::Error::new(
                    magnus::exception::exception(),
                    format!("Error flushing {:?}", err),
                )
            })?;
        Ok(())
    }
}

impl ItsiGrpcStream {
    pub fn new(response_sender: Sender<ByteFrame>, body: BodyDataStream<Incoming>) -> Self {
        let (trailer_tx, trailer_rx) = oneshot::channel::<HeaderMap>();
        ItsiGrpcStream {
            inner: Arc::new(Mutex::new(ItsiGrpcStreamInner {
                body,
                buf: Vec::new(),
                response_sender,
                response: Some(Response::new(BoxBody::new(Empty::new()))),
                trailer_tx,
                trailer_rx: Some(trailer_rx),
            })),
        }
    }

    pub fn read(&self, bytes: usize) -> MagnusResult<Bytes> {
        self.inner.lock().read(bytes)
    }

    pub fn write(&self, bytes: Bytes) -> MagnusResult<()> {
        self.inner.lock().write(bytes)
    }

    pub fn flush(&self) -> MagnusResult<()> {
        self.inner.lock().flush()
    }

    pub fn send_trailers(&self, trailers: HashMap<String, String>) -> MagnusResult<()> {
        self.inner.lock().send_trailers(trailers)
    }

    pub async fn build_response(
        &self,
        first_frame: ByteFrame,
        receiver: Receiver<ByteFrame>,
        shutdown_rx: watch::Receiver<RunningPhase>,
    ) -> HttpResponse {
        let mut response = self.inner.lock().response.take().unwrap();
        let rx = self.inner.lock().trailer_rx.take().unwrap();
        response
            .headers_mut()
            .append(CONTENT_TYPE, "application/grpc".parse().unwrap());
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
                                        warn!("Disconnecting streaming client.");
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
