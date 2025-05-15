use bytes::{Buf, Bytes};
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
use itsi_rb_helpers::call_without_gvl;
use itsi_tracing::error;
use magnus::error::Result as MagnusResult;
use memchr::{memchr, memchr_iter};
use parking_lot::RwLock;
use std::{
    collections::HashMap,
    io,
    ops::Deref,
    os::{fd::FromRawFd, unix::net::UnixStream},
    str::FromStr,
    sync::Arc,
};
use tokio::{
    io::AsyncReadExt,
    net::UnixStream as TokioUnixStream,
    sync::{mpsc::Sender, oneshot::Sender as OneshotSender, watch},
};

use tokio_util::io::ReaderStream;
use tracing::{debug, info, warn};

use crate::server::{
    http_message_types::{HttpBody, HttpResponse},
    serve_strategy::single_mode::RunningPhase,
};

#[magnus::wrap(class = "Itsi::HttpResponse", free_immediately, size)]
#[derive(Debug, Clone)]
pub struct ItsiHttpResponse {
    pub inner: Arc<ResponseInner>,
}

impl Deref for ItsiHttpResponse {
    type Target = Arc<ResponseInner>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

#[derive(Debug)]
pub struct ResponseInner {
    pub frame_writer: RwLock<Option<Sender<Bytes>>>,
    pub response: RwLock<Option<HttpResponse>>,
    pub hijacked_socket: RwLock<Option<UnixStream>>,
    pub response_sender: RwLock<Option<OneshotSender<ResponseFrame>>>,
    pub shutdown_rx: watch::Receiver<RunningPhase>,
    pub parts: Arc<Parts>,
}

#[derive(Debug)]
pub enum ResponseFrame {
    HttpResponse(HttpResponse),
    HijackedResponse(ItsiHttpResponse),
}

impl ItsiHttpResponse {
    pub fn new(
        parts: Arc<Parts>,
        response_sender: OneshotSender<ResponseFrame>,
        shutdown_rx: watch::Receiver<RunningPhase>,
    ) -> Self {
        Self {
            inner: Arc::new(ResponseInner {
                parts,
                shutdown_rx,
                response_sender: RwLock::new(Some(response_sender)),
                frame_writer: RwLock::new(None),
                response: RwLock::new(Some(Response::new(HttpBody::empty()))),
                hijacked_socket: RwLock::new(None),
            }),
        }
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
            self.hijacked_socket
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
            let parts = self.parts.clone();
            tokio::spawn(async move {
                let mut req = Request::from_parts((*parts).clone(), Empty::<Bytes>::new());
                match hyper::upgrade::on(&mut req).await {
                    Ok(upgraded) => {
                        Self::two_way_bridge(upgraded, reader)
                            .await
                            .expect("Error in creating two way bridge");
                    }
                    Err(e) => eprintln!("upgrade error: {:?}", e),
                }
            });
            Response::new(HttpBody::empty())
        } else {
            let stream = ReaderStream::new(reader);
            let boxed_body = if headers
                .get(TRANSFER_ENCODING)
                .is_some_and(|h| h == "chunked")
            {
                HttpBody::stream(unfold(
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
                                return Some((Ok(Bytes::from(data)), (stream, buf)));
                            }
                            match stream.next().await {
                                Some(Ok(chunk)) => buf.extend_from_slice(&chunk),
                                _ => return None,
                            }
                        }
                    },
                ))
            } else {
                HttpBody::stream(stream.map(|result: std::result::Result<Bytes, io::Error>| {
                    result.map_err(|e| unreachable!("unexpected io error: {:?}", e))
                }))
            };
            Response::new(boxed_body)
        };

        *response.status_mut() = status;
        *response.headers_mut() = headers;
        Ok(response)
    }

    pub fn internal_server_error(&self, message: String) {
        error!(message);
        self.close_write().ok();
        if let Some(mut response) = self.response.write().take() {
            *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
            if let Some(sender) = self.response_sender.write().take() {
                sender.send(ResponseFrame::HttpResponse(response)).ok();
            }
        }
    }

    pub fn send_frame(&self, frame: Bytes) -> MagnusResult<()> {
        {
            if self.frame_writer.read().is_none() && self.response.read().is_some() {
                if let Some(mut response) = self.response.write().take() {
                    let (writer, mut reader) = tokio::sync::mpsc::channel(5);
                    let mut shutdown_rx = self.shutdown_rx.clone();

                    let frame_stream = async_stream::stream! {
                        loop {
                            tokio::select! {
                                maybe_bytes = reader.recv() => {
                                    match maybe_bytes {
                                        Some(bytes) => {
                                            yield Ok(bytes);
                                        }
                                        _ => break,
                                    }
                                },
                                _ = shutdown_rx.changed() => {
                                    if *shutdown_rx.borrow() == RunningPhase::ShutdownPending {
                                        reader.close();
                                        while let Some(bytes) = reader.recv().await{
                                          yield Ok(bytes);
                                        }
                                        debug!("Disconnecting streaming client.");
                                        break;
                                    }
                                }
                            }
                        }
                    };

                    *response.body_mut() = HttpBody::stream(frame_stream);
                    self.frame_writer.write().replace(writer);
                    if let Some(sender) = self.response_sender.write().take() {
                        sender.send(ResponseFrame::HttpResponse(response)).ok();
                    }
                } else {
                    info!("No response!");
                }
            }
        }
        if let Some(frame_writer) = self.frame_writer.read().as_ref() {
            call_without_gvl(|| frame_writer.blocking_send(frame))
                .map_err(|_| itsi_error::ItsiError::ClientConnectionClosed)?;
        }
        Ok(())
    }

    pub fn send_and_close(&self, frame: Bytes) -> MagnusResult<()> {
        if self.frame_writer.read().is_some() {
            self.send_frame(frame)?;
            self.close()?;
            return Ok(());
        }
        if let Some(mut response) = self.response.write().take() {
            if frame.is_empty() {
                *response.body_mut() = HttpBody::empty();
            } else {
                *response.body_mut() = HttpBody::full(frame);
            }
            if let Some(sender) = self.response_sender.write().take() {
                sender.send(ResponseFrame::HttpResponse(response)).ok();
            }
        }

        Ok(())
    }

    pub fn close_write(&self) -> MagnusResult<bool> {
        self.frame_writer.write().take();
        Ok(true)
    }

    pub fn recv_frame(&self) {
        // not implemented
    }

    pub fn flush(&self) {
        // no-op
    }

    pub fn is_closed(&self) -> bool {
        self.response.read().is_none() && self.frame_writer.read().is_none()
    }

    pub fn is_hijacked(&self) -> bool {
        self.hijacked_socket.read().is_some()
    }

    pub fn close(&self) -> MagnusResult<()> {
        self.close_write()?;
        self.close_read()?;
        Ok(())
    }

    pub fn accept_str(&self) -> &str {
        self.parts
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

    pub fn reserve_headers(&self, header_count: usize) -> MagnusResult<()> {
        if let Some(ref mut resp) = *self.response.write() {
            resp.headers_mut().try_reserve(header_count).ok();
        }
        Ok(())
    }

    pub fn insert_header(
        &self,
        headers_mut: &mut HeaderMap,
        header_name: &HeaderName,
        value: Bytes,
    ) {
        static MAX_SPLIT_HEADERS: usize = 100;

        let mut start = 0usize;
        let mut emitted = 0usize;

        for idx in memchr_iter(b'\n', &value).chain(std::iter::once(value.len())) {
            if idx == start {
                start += 1;
                continue;
            }

            let mut part = value.slice(start..idx);
            if part.ends_with(b"\r") {
                part.truncate(part.len() - 1);
            }
            if let Some(&(b' ' | b'\t')) = part.first() {
                part.advance(1);
            }
            if memchr(0, &part).is_some() || part.iter().any(|&b| b < 0x20) {
                warn!("stripped control char from header {:?}", header_name);
                start = idx + 1;
                continue;
            }

            emitted += 1;
            if emitted > MAX_SPLIT_HEADERS {
                break;
            }

            let hv = unsafe { HeaderValue::from_maybe_shared_unchecked(part) };
            headers_mut.append(header_name, hv);
            start = idx + 1;
        }
    }

    pub fn add_header(&self, header_name: Bytes, value: Bytes) -> MagnusResult<()> {
        if let Some(ref mut resp) = *self.response.write() {
            let headers_mut = resp.headers_mut();
            let header_name = HeaderName::from_bytes(&header_name).map_err(|e| {
                itsi_error::ItsiError::InvalidInput(format!(
                    "Invalid header name {:?}: {:?}",
                    header_name, e
                ))
            })?;
            self.insert_header(headers_mut, &header_name, value);
        }
        Ok(())
    }

    pub fn add_headers(&self, headers: HashMap<Bytes, Vec<Bytes>>) -> MagnusResult<()> {
        if let Some(ref mut resp) = *self.response.write() {
            let headers_mut = resp.headers_mut();
            for (name, values) in headers {
                let header_name = HeaderName::from_bytes(&name).map_err(|e| {
                    itsi_error::ItsiError::InvalidInput(format!(
                        "Invalid header name {:?}: {:?}",
                        name, e
                    ))
                })?;
                for value in values {
                    self.insert_header(headers_mut, &header_name, value);
                }
            }
        }

        Ok(())
    }

    pub fn set_status(&self, status: u16) -> MagnusResult<()> {
        if let Some(ref mut resp) = *self.response.write() {
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

        *self.hijacked_socket.write() = Some(stream);
        if let Some(sender) = self.response_sender.write().take() {
            sender
                .send(ResponseFrame::HijackedResponse(self.clone()))
                .ok();
        }

        self.close()?;
        Ok(())
    }
}
