use super::bind::{Bind, BindAddress};
use super::bind_protocol::BindProtocol;
use super::io_stream::IoStream;
use itsi_error::Result;
use itsi_tracing::info;
use socket2::{Domain, Protocol, Socket, Type};
use std::net::{IpAddr, SocketAddr, TcpListener};
use std::sync::Arc;
use std::{os::unix::net::UnixListener, path::PathBuf};
use tokio::net::TcpListener as TokioTcpListener;
use tokio::net::UnixListener as TokioUnixListener;
use tokio::net::{unix, TcpStream, UnixStream};
use tokio_rustls::TlsAcceptor;

pub(crate) enum Listener {
    Tcp(TcpListener),
    TcpTls((TcpListener, TlsAcceptor)),
    Unix(UnixListener),
    UnixTls((UnixListener, TlsAcceptor)),
}

pub(crate) enum TokioListener {
    Tcp(TokioTcpListener),
    TcpTls((TokioTcpListener, TlsAcceptor)),
    Unix(TokioUnixListener),
    UnixTls((TokioUnixListener, TlsAcceptor)),
}

impl TokioListener {
    pub(crate) async fn accept(&self) -> Result<IoStream> {
        match self {
            TokioListener::Tcp(listener) => TokioListener::accept_tcp(listener).await,
            TokioListener::TcpTls((listener, acceptor)) => {
                TokioListener::accept_tls(listener, acceptor).await
            }
            TokioListener::Unix(stream) => TokioListener::accept_unix(stream).await,
            TokioListener::UnixTls((listener, acceptor)) => {
                TokioListener::accept_unix_tls(listener, acceptor).await
            }
        }
    }

    async fn accept_tcp(listener: &TokioTcpListener) -> Result<IoStream> {
        let tcp_stream = listener.accept().await?;
        Self::to_tokio_io(Stream::TcpStream(tcp_stream), None).await
    }

    async fn accept_tls(listener: &TokioTcpListener, acceptor: &TlsAcceptor) -> Result<IoStream> {
        let tcp_stream = listener.accept().await?;
        Self::to_tokio_io(Stream::TcpStream(tcp_stream), Some(acceptor)).await
    }

    async fn accept_unix(listener: &TokioUnixListener) -> Result<IoStream> {
        let unix_stream = listener.accept().await?;
        Self::to_tokio_io(Stream::UnixStream(unix_stream), None).await
    }

    async fn accept_unix_tls(
        listener: &TokioUnixListener,
        acceptor: &TlsAcceptor,
    ) -> Result<IoStream> {
        let unix_stream = listener.accept().await?;
        Self::to_tokio_io(Stream::UnixStream(unix_stream), Some(acceptor)).await
    }

    async fn to_tokio_io(
        input_stream: Stream,
        tls_acceptor: Option<&TlsAcceptor>,
    ) -> Result<IoStream> {
        match tls_acceptor {
            Some(acceptor) => match input_stream {
                Stream::TcpStream((tcp_stream, socket_address)) => {
                    match acceptor.accept(tcp_stream).await {
                        Ok(tls_stream) => Ok(IoStream::TcpTls {
                            stream: tls_stream,
                            addr: SockAddr::Tcp(Arc::new(socket_address)),
                        }),
                        Err(err) => Err(err.into()),
                    }
                }
                Stream::UnixStream((unix_stream, socket_address)) => {
                    match acceptor.accept(unix_stream).await {
                        Ok(tls_stream) => Ok(IoStream::UnixTls {
                            stream: tls_stream,
                            addr: SockAddr::Unix(Arc::new(socket_address)),
                        }),
                        Err(err) => Err(err.into()),
                    }
                }
            },
            None => match input_stream {
                Stream::TcpStream((tcp_stream, socket_address)) => Ok(IoStream::Tcp {
                    stream: tcp_stream,
                    addr: SockAddr::Tcp(Arc::new(socket_address)),
                }),
                Stream::UnixStream((unix_stream, socket_address)) => Ok(IoStream::Unix {
                    stream: unix_stream,
                    addr: SockAddr::Unix(Arc::new(socket_address)),
                }),
            },
        }
    }

    pub(crate) fn scheme(&self) -> String {
        match self {
            TokioListener::Tcp(_) => "http".to_string(),
            TokioListener::TcpTls(_) => "https".to_string(),
            TokioListener::Unix(_) => "http".to_string(),
            TokioListener::UnixTls(_) => "https".to_string(),
        }
    }

    pub(crate) fn port(&self) -> u16 {
        match self {
            TokioListener::Tcp(listener) => listener.local_addr().unwrap().port(),
            TokioListener::TcpTls((listener, _)) => listener.local_addr().unwrap().port(),
            TokioListener::Unix(_) => 0,
            TokioListener::UnixTls(_) => 0,
        }
    }

    pub(crate) fn host(&self) -> String {
        match self {
            TokioListener::Tcp(listener) => listener.local_addr().unwrap().ip().to_string(),
            TokioListener::TcpTls((listener, _)) => listener.local_addr().unwrap().ip().to_string(),
            TokioListener::Unix(_) => "unix".to_string(),
            TokioListener::UnixTls(_) => "unix".to_string(),
        }
    }
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
    pub fn to_tokio_listener(&self) -> TokioListener {
        match self {
            Listener::Tcp(listener) => TokioListener::Tcp(
                TokioTcpListener::from_std(TcpListener::try_clone(listener).unwrap()).unwrap(),
            ),
            Listener::TcpTls((listener, acceptor)) => TokioListener::TcpTls((
                TokioTcpListener::from_std(TcpListener::try_clone(listener).unwrap()).unwrap(),
                acceptor.clone(),
            )),
            Listener::Unix(listener) => TokioListener::Unix(
                TokioUnixListener::from_std(UnixListener::try_clone(listener).unwrap()).unwrap(),
            ),
            Listener::UnixTls((listener, acceptor)) => TokioListener::UnixTls((
                TokioUnixListener::from_std(UnixListener::try_clone(listener).unwrap()).unwrap(),
                acceptor.clone(),
            )),
        }
    }
}

impl TryFrom<Bind> for Listener {
    type Error = itsi_error::ItsiError;

    fn try_from(bind: Bind) -> std::result::Result<Self, Self::Error> {
        let bound = match bind.address {
            BindAddress::Ip(addr) => match bind.protocol {
                BindProtocol::Http => Listener::Tcp(connect_tcp_socket(addr, bind.port.unwrap())?),
                BindProtocol::Https => {
                    let tcp_listener = connect_tcp_socket(addr, bind.port.unwrap())?;
                    let tls_acceptor = TlsAcceptor::from(Arc::new(bind.tls_config.unwrap()));
                    Listener::TcpTls((tcp_listener, tls_acceptor))
                }
                _ => unreachable!(),
            },
            BindAddress::UnixSocket(path) => match bind.tls_config {
                Some(tls_config) => {
                    let tls_acceptor = TlsAcceptor::from(Arc::new(tls_config));
                    Listener::UnixTls((connect_unix_socket(&path)?, tls_acceptor))
                }
                None => Listener::Unix(connect_unix_socket(&path)?),
            },
        };
        Ok(bound)
    }
}

fn connect_tcp_socket(addr: IpAddr, port: u16) -> Result<TcpListener> {
    let domain = match addr {
        IpAddr::V4(_) => Domain::IPV4,
        IpAddr::V6(_) => Domain::IPV6,
    };
    let socket = Socket::new(domain, Type::STREAM, Some(Protocol::TCP))?;
    let socket_address: SocketAddr = SocketAddr::new(addr, port);
    socket.set_reuse_address(true).ok();
    socket.set_reuse_port(true).ok();
    socket.set_nonblocking(true).ok();
    socket.set_nodelay(true).ok();
    socket.set_recv_buffer_size(1_048_576).ok();
    socket.bind(&socket_address.into())?;
    socket.listen(1024)?;
    info!("Listening to {}", socket_address);
    Ok(socket.into())
}

fn connect_unix_socket(path: &PathBuf) -> Result<UnixListener> {
    let _ = std::fs::remove_file(path);
    let socket = Socket::new(Domain::UNIX, Type::STREAM, None)?;
    socket.set_nonblocking(true).ok();
    let socket_address = socket2::SockAddr::unix(path)?;

    info!("Binding to {:?}", path);
    socket.bind(&socket_address)?;
    socket.listen(1024)?;

    Ok(socket.into())
}
