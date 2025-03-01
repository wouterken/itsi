use super::bind::{Bind, BindAddress};
use super::transfer_protocol::TransferProtocol;
use hyper_util::rt::TokioIo;
use itsi_error::Result;
use itsi_tracing::info;
use socket2::{Domain, Protocol, Socket, Type};
use std::net::{IpAddr, SocketAddr, TcpListener as StdTcpListener};
use std::pin::Pin;
use std::sync::Arc;
use std::{os::unix::net::UnixListener as StdUnixListener, path::PathBuf};
use tokio::net::{unix, TcpListener, TcpStream, UnixListener, UnixStream};
use tokio_rustls::TlsAcceptor;

pub(crate) trait IoStream:
    tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin
{
}
impl<T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin> IoStream for T {}

pub(crate) enum Listener {
    Tcp(TcpListener),
    TcpTls((TcpListener, TlsAcceptor)),
    Unix(UnixListener),
    UnixTls((UnixListener, TlsAcceptor)),
}

enum Stream {
    TcpStream((TcpStream, SocketAddr)),
    UnixStream((UnixStream, unix::SocketAddr)),
}

#[derive(Clone, Debug)]
pub enum SockAddr {
    Tcp(Arc<SocketAddr>),
    Unix(Arc<unix::SocketAddr>),
}
impl std::fmt::Display for SockAddr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SockAddr::Tcp(socket_addr) => write!(f, "{}", socket_addr.ip().to_canonical()),
            SockAddr::Unix(socket_addr) => match socket_addr.as_pathname() {
                Some(path) => write!(f, "{:?}", path),
                None => write!(f, ""),
            },
        }
    }
}

impl Listener {
    pub(crate) async fn accept(&self) -> Result<(TokioIo<Pin<Box<dyn IoStream>>>, SockAddr)> {
        match self {
            Listener::Tcp(listener) => Listener::accept_tcp(listener).await,
            Listener::TcpTls((listener, acceptor)) => {
                Listener::accept_tls(listener, acceptor).await
            }
            Listener::Unix(stream) => Listener::accept_unix(stream).await,
            Listener::UnixTls((listener, acceptor)) => {
                Listener::accept_unix_tls(listener, acceptor).await
            }
        }
    }

    async fn to_tokio_io(
        input_stream: Stream,
        tls_acceptor: Option<&TlsAcceptor>,
    ) -> Result<(TokioIo<Pin<Box<dyn IoStream>>>, SockAddr)> {
        match tls_acceptor {
            Some(acceptor) => match input_stream {
                Stream::TcpStream((tcp_stream, socket_address)) => {
                    match acceptor.accept(tcp_stream).await {
                        Ok(tls_stream) => Ok((
                            TokioIo::new(Box::pin(tls_stream) as Pin<Box<dyn IoStream>>),
                            SockAddr::Tcp(Arc::new(socket_address)),
                        )),
                        Err(err) => Err(err.into()),
                    }
                }
                Stream::UnixStream((unix_stream, socket_address)) => {
                    match acceptor.accept(unix_stream).await {
                        Ok(tls_stream) => Ok((
                            TokioIo::new(Box::pin(tls_stream) as Pin<Box<dyn IoStream>>),
                            SockAddr::Unix(Arc::new(socket_address)),
                        )),
                        Err(err) => Err(err.into()),
                    }
                }
            },
            None => match input_stream {
                Stream::TcpStream((tcp_stream, socket_address)) => Ok((
                    TokioIo::new(Box::pin(tcp_stream) as Pin<Box<dyn IoStream>>),
                    SockAddr::Tcp(Arc::new(socket_address)),
                )),
                Stream::UnixStream((unix_stream, socket_address)) => Ok((
                    TokioIo::new(Box::pin(unix_stream) as Pin<Box<dyn IoStream>>),
                    SockAddr::Unix(Arc::new(socket_address)),
                )),
            },
        }
    }

    async fn accept_tcp(
        listener: &TcpListener,
    ) -> Result<(TokioIo<Pin<Box<dyn IoStream>>>, SockAddr)> {
        let tcp_stream = listener.accept().await?;
        Self::to_tokio_io(Stream::TcpStream(tcp_stream), None).await
    }

    async fn accept_tls(
        listener: &TcpListener,
        acceptor: &TlsAcceptor,
    ) -> Result<(TokioIo<Pin<Box<dyn IoStream>>>, SockAddr)> {
        let tcp_stream = listener.accept().await?;
        Self::to_tokio_io(Stream::TcpStream(tcp_stream), Some(acceptor)).await
    }

    async fn accept_unix(
        listener: &UnixListener,
    ) -> Result<(TokioIo<Pin<Box<dyn IoStream>>>, SockAddr)> {
        let unix_stream = listener.accept().await?;
        Self::to_tokio_io(Stream::UnixStream(unix_stream), None).await
    }

    async fn accept_unix_tls(
        listener: &UnixListener,
        acceptor: &TlsAcceptor,
    ) -> Result<(TokioIo<Pin<Box<dyn IoStream>>>, SockAddr)> {
        let unix_stream = listener.accept().await?;
        Self::to_tokio_io(Stream::UnixStream(unix_stream), Some(acceptor)).await
    }

    pub(crate) fn scheme(&self) -> String {
        match self {
            Listener::Tcp(_) => "http".to_string(),
            Listener::TcpTls(_) => "https".to_string(),
            Listener::Unix(_) => "http".to_string(),
            Listener::UnixTls(_) => "https".to_string(),
        }
    }

    pub(crate) fn port(&self) -> u16 {
        match self {
            Listener::Tcp(listener) => listener.local_addr().unwrap().port(),
            Listener::TcpTls((listener, _)) => listener.local_addr().unwrap().port(),
            Listener::Unix(_) => 0,
            Listener::UnixTls(_) => 0,
        }
    }

    pub(crate) fn host(&self) -> String {
        match self {
            Listener::Tcp(listener) => listener.local_addr().unwrap().ip().to_string(),
            Listener::TcpTls((listener, _)) => listener.local_addr().unwrap().ip().to_string(),
            Listener::Unix(_) => "unix".to_string(),
            Listener::UnixTls(_) => "unix".to_string(),
        }
    }
}

impl From<Bind> for Listener {
    fn from(bind: Bind) -> Self {
        match bind.address {
            BindAddress::Ip(addr) => match bind.protocol {
                TransferProtocol::Http => Listener::Tcp(
                    TcpListener::from_std(connect_tcp_socket(addr, bind.port.unwrap())).unwrap(),
                ),
                TransferProtocol::Https => {
                    let tcp_listener =
                        TcpListener::from_std(connect_tcp_socket(addr, bind.port.unwrap()))
                            .unwrap();
                    let tls_acceptor = TlsAcceptor::from(Arc::new(bind.tls_config.unwrap()));
                    Listener::TcpTls((tcp_listener, tls_acceptor))
                }
                _ => unreachable!(),
            },
            BindAddress::UnixSocket(path) => match bind.tls_config {
                Some(tls_config) => {
                    let tls_acceptor = TlsAcceptor::from(Arc::new(tls_config));
                    Listener::UnixTls((
                        UnixListener::from_std(connect_unix_socket(&path)).unwrap(),
                        tls_acceptor,
                    ))
                }
                None => Listener::Unix(UnixListener::from_std(connect_unix_socket(&path)).unwrap()),
            },
        }
    }
}

fn connect_tcp_socket(addr: IpAddr, port: u16) -> StdTcpListener {
    let domain = match addr {
        IpAddr::V4(_) => Domain::IPV4,
        IpAddr::V6(_) => Domain::IPV6,
    };
    let socket = Socket::new(domain, Type::STREAM, Some(Protocol::TCP)).unwrap();
    let socket_address: SocketAddr = SocketAddr::new(addr, port);
    socket.set_reuse_address(true).ok();
    socket.set_reuse_port(true).ok();
    socket.set_nonblocking(true).ok();
    socket.set_nodelay(true).ok();
    socket.set_recv_buffer_size(1_048_576).ok();
    info!("Binding to {}", socket_address);
    socket.bind(&socket_address.into()).unwrap();
    socket.listen(1024).unwrap();
    socket.into()
}

fn connect_unix_socket(path: &PathBuf) -> StdUnixListener {
    let _ = std::fs::remove_file(path);
    let socket = Socket::new(Domain::UNIX, Type::STREAM, None).unwrap();
    socket.set_nonblocking(true).ok();
    let socket_address = socket2::SockAddr::unix(path).unwrap();

    info!("Binding to {:?}", path);
    socket.bind(&socket_address).unwrap();
    socket.listen(1024).unwrap();

    socket.into()
}
