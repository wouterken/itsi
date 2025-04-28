use bytes::{Bytes, BytesMut};
use derive_more::Debug;
use futures::stream::{unfold, StreamExt};
use http::{
    header::{ACCEPT, TRANSFER_ENCODING},
    request::Parts,
    HeaderMap, HeaderName, HeaderValue, Request, Response, StatusCode,
};
use http_body_util::{combinators::BoxBody, Empty, Full, StreamBody};
use hyper::{body::Frame, upgrade::Upgraded};
use hyper_util::rt::TokioIo;
use itsi_error::Result;
use itsi_tracing::error;
use magnus::error::Result as MagnusResult;
use parking_lot::RwLock;
use std::{
    collections::HashMap,
    io,
    os::{fd::FromRawFd, unix::net::UnixStream},
    str::FromStr,
    sync::Arc,
};
use tokio::{
    io::AsyncReadExt,
    net::UnixStream as TokioUnixStream,
    sync::{
        mpsc::{self},
        watch,
    },
};
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::io::ReaderStream;
use tracing::warn;

use crate::server::{
    byte_frame::ByteFrame, http_message_types::HttpResponse,
    serve_strategy::single_mode::RunningPhase,
};

#[magnus::wrap(class = "Itsi::HttpResponse", free_immediately, size)]
#[derive(Debug, Clone)]
pub struct ItsiHttpResponse {
    pub data: Arc<ResponseData>,
}

#[derive(Debug)]
pub struct ResponseData {
    pub response: RwLock<Option<HttpResponse>>,
    pub response_writer: RwLock<Option<mpsc::Sender<ByteFrame>>>,
    pub response_buffer: RwLock<BytesMut>,
    pub hijacked_socket: RwLock<Option<UnixStream>>,
    pub parts: Parts,
}

impl ItsiHttpResponse {
    pub async fn build(
        &self,
        first_frame: ByteFrame,
        receiver: mpsc::Receiver<ByteFrame>,
        shutdown_rx: watch::Receiver<RunningPhase>,
    ) -> HttpResponse {
        if self.is_hijacked() {
            return match self.process_hijacked_response().await {
                Ok(result) => result,
                Err(e) => {
                    error!("Error processing hijacked response: {}", e);
                    Response::new(BoxBody::new(Empty::new()))
                }
            };
        }
        let mut response = self.data.response.write().take().unwrap();
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
        };
        response
    }

    pub fn close(&self) {
        self.data.response_writer.write().take();
    }

    async fn two_way_bridge(upgraded: Upgraded, local: TokioUnixStream) -> io::Result<()> {
        let client_io = TokioIo::new(upgraded);

        // Split each side
        let (mut lr, mut lw) = tokio::io::split(local);
        let (mut cr, mut cw) = tokio::io::split(client_io);

        let to_ruby = tokio::spawn(async move {
            if let Err(e) = tokio::io::copy(&mut cr, &mut lw).await {
                eprintln!("Error copying upgraded->local: {:?}", e);
            }
        });
        let from_ruby = tokio::spawn(async move {
            if let Err(e) = tokio::io::copy(&mut lr, &mut cw).await {
                eprintln!("Error copying upgraded->local: {:?}", e);
            }
        });

        let _ = to_ruby.await;
        let _ = from_ruby.await;
        Ok(())
    }

    async fn read_response_headers(&self, reader: &mut TokioUnixStream) -> Result<Vec<u8>> {
        let mut buf = [0u8; 1];
        let mut collected = Vec::new();
        loop {
            let n = reader.read(&mut buf).await?;
            if n == 0 {
                // EOF reached unexpectedly
                break;
            }
            collected.push(buf[0]);
            if collected.ends_with(b"\r\n\r\n") {
                break;
            }
        }

        Ok(collected)
    }

    pub async fn read_hijacked_headers(
        &self,
    ) -> Result<(HeaderMap, StatusCode, bool, TokioUnixStream)> {
        let hijacked_socket =
            self.data
                .hijacked_socket
                .write()
                .take()
                .ok_or(itsi_error::ItsiError::InvalidInput(
                    "Couldn't hijack stream".to_owned(),
                ))?;
        let mut reader = TokioUnixStream::from_std(hijacked_socket).unwrap();
        let response_headers = self.read_response_headers(&mut reader).await?;
        let mut headers = [httparse::EMPTY_HEADER; 64];
        let mut resp = httparse::Response::new(&mut headers);
        resp.parse(&response_headers)?;

        let status = StatusCode::from_u16(resp.code.unwrap_or(200)).unwrap_or(StatusCode::OK);
        let mut headers = HeaderMap::new();
        for header in resp.headers.iter() {
            headers.insert(
                HeaderName::from_str(header.name).unwrap(),
                HeaderValue::from_bytes(header.value).unwrap(),
            );
        }
        let requires_upgrade = status == StatusCode::SWITCHING_PROTOCOLS;
        Ok((headers, status, requires_upgrade, reader))
    }

    pub async fn process_hijacked_response(&self) -> Result<HttpResponse> {
        let (headers, status, requires_upgrade, reader) = self.read_hijacked_headers().await?;
        let mut response = if requires_upgrade {
            let parts = self.data.parts.clone();
            tokio::spawn(async move {
                let mut req = Request::from_parts(parts, Empty::<Bytes>::new());
                match hyper::upgrade::on(&mut req).await {
                    Ok(upgraded) => {
                        Self::two_way_bridge(upgraded, reader)
                            .await
                            .expect("Error in creating two way bridge");
                    }
                    Err(e) => eprintln!("upgrade error: {:?}", e),
                }
            });
            Response::new(BoxBody::new(Empty::new()))
        } else {
            let stream = ReaderStream::new(reader);
            let boxed_body = if headers
                .get(TRANSFER_ENCODING)
                .is_some_and(|h| h == "chunked")
            {
                BoxBody::new(StreamBody::new(unfold(
                    (stream, Vec::new()),
                    |(mut stream, mut buf)| async move {
                        loop {
                            if let Some(pos) = buf.iter().position(|&b| b == b'\n') {
                                let line = buf.drain(..=pos).collect::<Vec<u8>>();
                                let line = std::str::from_utf8(&line).ok()?.trim();
                                let chunk_size = usize::from_str_radix(line, 16).ok()?;
                                if chunk_size == 0 {
                                    return None;
                                }
                                while buf.len() < chunk_size {
                                    match stream.next().await {
                                        Some(Ok(chunk)) => buf.extend_from_slice(&chunk),
                                        _ => return None,
                                    }
                                }
                                let data = buf.drain(..chunk_size).collect::<Vec<u8>>();
                                if buf.starts_with(b"\r\n") {
                                    buf.drain(..2);
                                }
                                return Some((Ok(Frame::data(Bytes::from(data))), (stream, buf)));
                            }
                            match stream.next().await {
                                Some(Ok(chunk)) => buf.extend_from_slice(&chunk),
                                _ => return None,
                            }
                        }
                    },
                )))
            } else {
                BoxBody::new(StreamBody::new(stream.map(
                    |result: std::result::Result<Bytes, io::Error>| {
                        result
                            .map(Frame::data)
                            .map_err(|e| unreachable!("unexpected io error: {:?}", e))
                    },
                )))
            };
            Response::new(boxed_body)
        };

        *response.status_mut() = status;
        *response.headers_mut() = headers;
        Ok(response)
    }

    pub fn internal_server_error(&self, message: String) {
        error!(message);
        self.data.response_writer.write().take();
        if let Some(ref mut response) = *self.data.response.write() {
            *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
        }
    }

    pub fn send_frame(&self, frame: Bytes) -> MagnusResult<()> {
        self.send_frame_into(ByteFrame::Data(frame), &self.data.response_writer)
    }

    pub fn recv_frame(&self) {
        // not implemented
    }

    pub fn flush(&self) {
        // no-op
    }

    pub fn is_closed(&self) -> bool {
        self.data.response_writer.write().is_none()
    }

    pub fn send_and_close(&self, frame: Bytes) -> MagnusResult<()> {
        let result = self.send_frame_into(ByteFrame::End(frame), &self.data.response_writer);
        self.data.response_writer.write().take();
        result
    }

    pub fn send_frame_into(
        &self,
        frame: ByteFrame,
        writer: &RwLock<Option<mpsc::Sender<ByteFrame>>>,
    ) -> MagnusResult<()> {
        if let Some(writer) = writer.write().as_ref() {
            return Ok(writer
                .blocking_send(frame)
                .map_err(|_| itsi_error::ItsiError::ClientConnectionClosed)?);
        }
        Ok(())
    }

    pub fn is_hijacked(&self) -> bool {
        self.data.hijacked_socket.read().is_some()
    }

    pub fn close_write(&self) -> MagnusResult<bool> {
        self.data.response_writer.write().take();
        Ok(true)
    }

    pub fn accept_str(&self) -> &str {
        self.data
            .parts
            .headers
            .get(ACCEPT)
            .and_then(|hv| hv.to_str().ok()) // handle invalid utf-8
            .unwrap_or("application/x-www-form-urlencoded")
    }

    pub fn is_html(&self) -> bool {
        self.accept_str().starts_with("text/html")
    }

    pub fn is_json(&self) -> bool {
        self.accept_str().starts_with("application/json")
    }

    pub fn close_read(&self) -> MagnusResult<bool> {
        Ok(true)
    }

    pub fn new(parts: Parts, response_writer: mpsc::Sender<ByteFrame>) -> Self {
        Self {
            data: Arc::new(ResponseData {
                response: RwLock::new(Some(Response::new(BoxBody::new(Empty::new())))),
                response_writer: RwLock::new(Some(response_writer)),
                response_buffer: RwLock::new(BytesMut::new()),
                hijacked_socket: RwLock::new(None),
                parts,
            }),
        }
    }

    pub fn add_header(&self, name: Bytes, value: Bytes) -> MagnusResult<()> {
        let header_name: HeaderName = HeaderName::from_bytes(&name).map_err(|e| {
            itsi_error::ItsiError::InvalidInput(format!("Invalid header name {:?}: {:?}", name, e))
        })?;
        let header_value = unsafe { HeaderValue::from_maybe_shared_unchecked(value) };
        if let Some(ref mut resp) = *self.data.response.write() {
            resp.headers_mut().append(header_name, header_value);
        }
        Ok(())
    }

    pub fn add_headers(&self, headers: HashMap<Bytes, Vec<Bytes>>) -> MagnusResult<()> {
        if let Some(ref mut resp) = *self.data.response.write() {
            let headers_mut = resp.headers_mut();
            for (name, values) in headers {
                let header_name = HeaderName::from_bytes(&name).map_err(|e| {
                    itsi_error::ItsiError::InvalidInput(format!(
                        "Invalid header name {:?}: {:?}",
                        name, e
                    ))
                })?;
                for value in values {
                    let header_value = unsafe { HeaderValue::from_maybe_shared_unchecked(value) };
                    headers_mut.append(&header_name, header_value);
                }
            }
        }

        Ok(())
    }

    pub fn set_status(&self, status: u16) -> MagnusResult<()> {
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

    pub fn hijack(&self, fd: i32) -> MagnusResult<()> {
        let stream = unsafe { UnixStream::from_raw_fd(fd) };

        *self.data.hijacked_socket.write() = Some(stream);
        if let Some(writer) = self.data.response_writer.write().as_ref() {
            writer
                .blocking_send(ByteFrame::Empty)
                .map_err(|_| itsi_error::ItsiError::ClientConnectionClosed)?
        }
        self.close();
        Ok(())
    }
}
