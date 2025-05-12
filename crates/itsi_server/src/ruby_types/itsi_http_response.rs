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
use itsi_tracing::error;
use magnus::error::Result as MagnusResult;
use memchr::{memchr, memchr_iter};
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
    sync::{mpsc::Sender, watch, Notify},
};
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::io::ReaderStream;
use tracing::warn;

use crate::server::{http_message_types::HttpResponse, serve_strategy::single_mode::RunningPhase};

#[magnus::wrap(class = "Itsi::HttpResponse", free_immediately, size)]
#[derive(Debug, Clone)]
pub struct ItsiHttpResponse {
    pub data: Arc<RwLock<ResponseData>>,
    pub response_ready_notify: Arc<Notify>,
    pub shutdown_rx: watch::Receiver<RunningPhase>,
    pub parts: Arc<Parts>,
}

#[derive(Debug)]
pub struct ResponseData {
    pub response: Option<HttpResponse>,
    pub frame_writer: Option<Sender<Bytes>>,
    pub hijacked_socket: Option<UnixStream>,
}

impl ItsiHttpResponse {
    pub fn new(
        parts: Arc<Parts>,
        response_ready_notify: Arc<Notify>,
        shutdown_rx: watch::Receiver<RunningPhase>,
    ) -> Self {
        Self {
            parts,
            response_ready_notify,
            shutdown_rx,
            data: Arc::new(RwLock::new(ResponseData {
                response: Some(Response::new(BoxBody::new(Empty::new()))),
                frame_writer: None,
                hijacked_socket: None,
            })),
        }
    }

    pub async fn get_response(&self) -> HttpResponse {
        if self.is_hijacked() {
            return match self.process_hijacked_response().await {
                Ok(result) => result,
                Err(e) => {
                    error!("Error processing hijacked response: {}", e);
                    Response::new(BoxBody::new(Empty::new()))
                }
            };
        }

        self.data.write().response.take().unwrap()
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
                .write()
                .hijacked_socket
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
        self.close_write().ok();
        if let Some(ref mut response) = self.data.write().response {
            *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
            self.response_ready_notify.notify_one();
        }
    }

    pub fn send_frame(&self, frame: Bytes) -> MagnusResult<()> {
        {
            let mut data_guard = self.data.write();
            if data_guard.frame_writer.is_none() && data_guard.response.is_some() {
                if let Some(ref mut response) = data_guard.response {
                    let (writer, reader) = tokio::sync::mpsc::channel(5);
                    let mut shutdown_rx = self.shutdown_rx.clone();
                    let mut stream = ReceiverStream::new(reader);

                    let frame_stream = async_stream::stream! {
                        loop {
                            tokio::select! {
                                maybe_bytes = stream.next() => {
                                    match maybe_bytes {
                                        Some(bytes) => {
                                            yield Ok(Frame::data(bytes));
                                        }
                                        _ => break,
                                    }
                                },
                                _ = shutdown_rx.changed() => {
                                    if *shutdown_rx.borrow() == RunningPhase::ShutdownPending {
                                        warn!("Disconnecting streaming client.");
                                        break;
                                    }
                                }
                            }
                        }
                    };

                    *response.body_mut() = BoxBody::new(StreamBody::new(frame_stream));
                    data_guard.frame_writer.replace(writer);
                    self.response_ready_notify.notify_one();
                }
            }
        }
        if let Some(frame_writer) = self.data.read().frame_writer.as_ref() {
            frame_writer
                .blocking_send(frame)
                .map_err(|_| itsi_error::ItsiError::ClientConnectionClosed)?
        }
        Ok(())
    }

    pub fn send_and_close(&self, frame: Bytes) -> MagnusResult<()> {
        if self.data.read().frame_writer.is_some() {
            self.send_frame(frame)?;
            self.close()?;
            return Ok(());
        }
        if let Some(ref mut response) = self.data.write().response {
            *response.body_mut() = BoxBody::new(Full::new(frame));
            self.response_ready_notify.notify_one();
        }

        Ok(())
    }

    pub fn close_write(&self) -> MagnusResult<bool> {
        self.data.write().frame_writer.take();
        Ok(true)
    }

    pub fn recv_frame(&self) {
        // not implemented
    }

    pub fn flush(&self) {
        // no-op
    }

    pub fn is_closed(&self) -> bool {
        self.data.read().response.is_none() && self.data.read().frame_writer.is_none()
    }

    pub fn is_hijacked(&self) -> bool {
        self.data.read().hijacked_socket.is_some()
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
        if let Some(ref mut resp) = self.data.write().response {
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
        if let Some(ref mut resp) = self.data.write().response {
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
        if let Some(ref mut resp) = self.data.write().response {
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
        if let Some(ref mut resp) = self.data.write().response {
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

        self.data.write().hijacked_socket = Some(stream);
        self.response_ready_notify.notify_one();
        self.close()?;
        Ok(())
    }
}
