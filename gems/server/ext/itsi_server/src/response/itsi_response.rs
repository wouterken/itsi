use bytes::{Bytes, BytesMut};
use derive_more::Debug;
use futures::stream::StreamExt;
use http::{request::Parts, HeaderName, Response, StatusCode};
use http_body_util::{combinators::BoxBody, Empty, Full, StreamBody};
use itsi_tracing::error;
use magnus::error::Result;
use parking_lot::RwLock;
use std::{convert::Infallible, sync::Arc};
use tokio::sync::mpsc::{self, Receiver};

#[magnus::wrap(class = "Itsi::Response", free_immediately, size)]
#[derive(Debug, Clone)]
pub struct ItsiResponse {
    pub data: Arc<ResponseData>,
}

#[derive(Debug)]
pub struct ResponseData {
    pub response: RwLock<Option<Response<BoxBody<Bytes, Infallible>>>>,
    pub response_writer: RwLock<Option<mpsc::Sender<Bytes>>>,
    pub response_buffer: RwLock<BytesMut>,
}

impl ItsiResponse {
    pub fn build(
        &self,
        first_frame: Option<Bytes>,
        receiver: mpsc::Receiver<Bytes>,
    ) -> Response<BoxBody<Bytes, Infallible>> {
        let mut response = self.data.response.write().take().unwrap();
        *response.body_mut() = if first_frame.is_none() {
            BoxBody::new(Empty::new())
        } else if receiver.is_closed() && receiver.is_empty() {
            BoxBody::new(Full::new(first_frame.unwrap()))
        } else {
            let initial_frame =
                tokio_stream::once(Ok(hyper::body::Frame::data(first_frame.unwrap())));
            let frame_stream = initial_frame.chain(
                tokio_stream::wrappers::ReceiverStream::new(receiver)
                    .map(|bytes| Ok(hyper::body::Frame::data(bytes))),
            );
            BoxBody::new(StreamBody::new(frame_stream))
        };
        response
    }

    pub fn close(&self) {
        self.data.response_writer.write().take();
    }

    pub fn error(&self, message: String) {
        error!(message);
        self.data.response_writer.write().take();
    }

    pub fn send_frame(&self, frame: Bytes) -> Result<usize> {
        self.send_frame_into(frame, &self.data.response_writer)
    }

    pub fn send_and_close(&self, frame: Bytes) -> Result<usize> {
        let result = self.send_frame_into(frame, &self.data.response_writer);
        self.data.response_writer.write().take();
        result
    }

    pub fn send_frame_into(
        &self,
        frame: Bytes,
        writer: &RwLock<Option<mpsc::Sender<Bytes>>>,
    ) -> Result<usize> {
        if let Some(writer) = writer.write().as_ref() {
            writer
                .blocking_send(frame)
                .map_err(|e| itsi_error::ItsiError::ClientConnectionClosed)?;
        }
        Ok(0)
    }

    pub fn close_write(&self) -> Result<bool> {
        self.data.response_writer.write().take();
        Ok(true)
    }

    pub async fn build_body(
        &self,
        first_frame: Option<Bytes>,
        receiver: Receiver<Bytes>,
    ) -> BoxBody<Bytes, Infallible> {
        match first_frame {
            Some(first_frame) => {
                if receiver.is_closed() && receiver.is_empty() {
                    BoxBody::new(Full::new(first_frame))
                } else {
                    let initial_frame =
                        tokio_stream::once(Ok(hyper::body::Frame::data(first_frame)));
                    let frame_stream = initial_frame.chain(
                        tokio_stream::wrappers::ReceiverStream::new(receiver)
                            .map(|bytes| Ok(hyper::body::Frame::data(bytes))),
                    );
                    BoxBody::new(StreamBody::new(frame_stream))
                }
            }
            None => BoxBody::new(Empty::new()),
        }
    }
    pub fn new(_parts: Arc<Parts>, response_writer: mpsc::Sender<Bytes>) -> Self {
        Self {
            data: Arc::new(ResponseData {
                response: RwLock::new(Some(Response::new(BoxBody::new(Empty::new())))),
                response_writer: RwLock::new(Some(response_writer)),
                response_buffer: RwLock::new(BytesMut::new()),
            }),
        }
    }

    pub fn add_header(&self, name: String, value: String) -> Result<()> {
        let header_name: HeaderName = name.parse().map_err(|e| {
            itsi_error::ItsiError::InvalidInput(format!("Invalid header name {:?}: {:?}", name, e))
        })?;
        let header_value = value.parse().map_err(|e| {
            itsi_error::ItsiError::InvalidInput(format!(
                "Invalid header value {:?}: {:?}",
                value, e
            ))
        })?;
        if let Some(ref mut resp) = *self.data.response.write() {
            resp.headers_mut().insert(header_name, header_value);
        }
        Ok(())
    }

    pub fn set_status(&self, status: u16) -> Result<()> {
        if let Some(ref mut resp) = *self.data.response.write() {
            *resp.status_mut() = StatusCode::from_u16(status).map_err(|e| {
                itsi_error::ItsiError::InvalidInput(format!(
                    "Invalid status code {:?}: {:?}",
                    status, e
                ))
            })?;
        }
        Ok(())
    }

    pub fn hijack(&self) -> Result<()> {
        Ok(())
    }
}
