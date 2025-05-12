use pin_project::pin_project;
use tokio::net::{TcpStream, UnixStream};
use tokio_rustls::server::TlsStream;

use std::os::unix::io::{AsRawFd, RawFd};
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite};

use super::binds::listener::SockAddr;

#[pin_project(project = IoStreamEnumProj)]
pub enum IoStream {
    Tcp {
        #[pin]
        stream: TcpStream,
        addr: SockAddr,
    },
    TcpTls {
        #[pin]
        stream: TlsStream<TcpStream>,
        addr: SockAddr,
    },
    Unix {
        #[pin]
        stream: UnixStream,
        addr: SockAddr,
    },
    UnixTls {
        #[pin]
        stream: TlsStream<UnixStream>,
        addr: SockAddr,
    },
}

impl IoStream {
    pub fn addr(&self) -> String {
        match self {
            IoStream::Tcp { addr, .. } => addr.to_string(),
            IoStream::TcpTls { addr, .. } => addr.to_string(),
            IoStream::Unix { addr, .. } => addr.to_string(),
            IoStream::UnixTls { addr, .. } => addr.to_string(),
        }
    }
}

impl AsyncRead for IoStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        match self.project() {
            IoStreamEnumProj::Tcp { stream, .. } => stream.poll_read(cx, buf),
            IoStreamEnumProj::TcpTls { stream, .. } => stream.poll_read(cx, buf),
            IoStreamEnumProj::Unix { stream, .. } => stream.poll_read(cx, buf),
            IoStreamEnumProj::UnixTls { stream, .. } => stream.poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for IoStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        match self.project() {
            IoStreamEnumProj::Tcp { stream, .. } => stream.poll_write(cx, buf),
            IoStreamEnumProj::TcpTls { stream, .. } => stream.poll_write(cx, buf),
            IoStreamEnumProj::Unix { stream, .. } => stream.poll_write(cx, buf),
            IoStreamEnumProj::UnixTls { stream, .. } => stream.poll_write(cx, buf),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match self.project() {
            IoStreamEnumProj::Tcp { stream, .. } => stream.poll_flush(cx),
            IoStreamEnumProj::TcpTls { stream, .. } => stream.poll_flush(cx),
            IoStreamEnumProj::Unix { stream, .. } => stream.poll_flush(cx),
            IoStreamEnumProj::UnixTls { stream, .. } => stream.poll_flush(cx),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match self.project() {
            IoStreamEnumProj::Tcp { stream, .. } => stream.poll_shutdown(cx),
            IoStreamEnumProj::TcpTls { stream, .. } => stream.poll_shutdown(cx),
            IoStreamEnumProj::Unix { stream, .. } => stream.poll_shutdown(cx),
            IoStreamEnumProj::UnixTls { stream, .. } => stream.poll_shutdown(cx),
        }
    }
}

impl AsRawFd for IoStream {
    fn as_raw_fd(&self) -> RawFd {
        // For immutable access, we can simply pattern-match on self.
        match self {
            IoStream::Tcp { stream, .. } => stream.as_raw_fd(),
            IoStream::TcpTls { stream, .. } => stream.get_ref().0.as_raw_fd(),
            IoStream::Unix { stream, .. } => stream.as_raw_fd(),
            IoStream::UnixTls { stream, .. } => stream.get_ref().0.as_raw_fd(),
        }
    }
}
