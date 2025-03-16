use super::bind::{Bind, BindAddress};
use super::bind_protocol::BindProtocol;
use super::io_stream::IoStream;
use super::serve_strategy::single_mode::RunningPhase;
use super::tls::ItsiTlsAcceptor;
use itsi_error::{ItsiError, Result};
use itsi_tracing::info;
use socket2::{Domain, Protocol, Socket, Type};
use std::net::{IpAddr, SocketAddr, TcpListener};
use std::sync::Arc;
use std::{os::unix::net::UnixListener, path::PathBuf};
use tokio::net::TcpListener as TokioTcpListener;
use tokio::net::UnixListener as TokioUnixListener;
use tokio::net::{unix, TcpStream, UnixStream};
use tokio::sync::watch::Receiver;
use tokio_rustls::TlsAcceptor;
use tokio_stream::StreamExt;
use tracing::error;

pub(crate) enum Listener {
    Tcp(TcpListener),
    TcpTls((TcpListener, ItsiTlsAcceptor)),
    Unix(UnixListener),
    UnixTls((UnixListener, ItsiTlsAcceptor)),
}

pub(crate) enum TokioListener {
    Tcp(TokioTcpListener),
    TcpTls(TokioTcpListener, ItsiTlsAcceptor),
    Unix(TokioUnixListener),
    UnixTls(TokioUnixListener, ItsiTlsAcceptor),
}

#[derive(Debug, Clone)]
pub struct ListenerInfo {
    pub host: String,
    pub port: u16,
    pub scheme: String,
}

impl TokioListener {
    pub fn listener_info(&self) -> ListenerInfo {
        match self {
            TokioListener::Tcp(listener) => ListenerInfo {
                host: listener
                    .local_addr()
                    .unwrap()
                    .ip()
                    .to_canonical()
                    .to_string(),
                port: listener.local_addr().unwrap().port(),
                scheme: "http".to_string(),
            },
            TokioListener::TcpTls(listener, _) => ListenerInfo {
                host: listener
                    .local_addr()
                    .unwrap()
                    .ip()
                    .to_canonical()
                    .to_string(),
                port: listener.local_addr().unwrap().port(),
                scheme: "https".to_string(),
            },
            TokioListener::Unix(listener) => ListenerInfo {
                host: listener
                    .local_addr()
                    .unwrap()
                    .as_pathname()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_owned(),
                port: 0,
                scheme: "unix".to_string(),
            },
            TokioListener::UnixTls(listener, _) => ListenerInfo {
                host: listener
                    .local_addr()
                    .unwrap()
                    .as_pathname()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_owned(),
                port: 0,
                scheme: "ssl".to_string(),
            },
        }
    }

    pub(crate) async fn accept(&self) -> Result<IoStream> {
        match self {
            TokioListener::Tcp(listener) => TokioListener::accept_tcp(listener).await,
            TokioListener::TcpTls(listener, acceptor) => {
                TokioListener::accept_tls(listener, acceptor).await
            }
            TokioListener::Unix(listener) => TokioListener::accept_unix(listener).await,
            TokioListener::UnixTls(listener, acceptor) => {
                TokioListener::accept_unix_tls(listener, acceptor).await
            }
        }
    }

    async fn accept_tcp(listener: &TokioTcpListener) -> Result<IoStream> {
        let tcp_stream = listener.accept().await?;
        Self::to_tokio_io(Stream::TcpStream(tcp_stream), None).await
    }

    pub async fn spawn_state_task(&self, mut shutdown_receiver: Receiver<RunningPhase>) {
        if let TokioListener::TcpTls(
            _,
            ItsiTlsAcceptor::Automatic(_acme_acceptor, state, _server_config),
        ) = self
        {
            let mut state = state.lock().await;
            loop {
                tokio::select! {
                  stream_event = StreamExt::next(&mut *state) => {
                      match stream_event {
                        Some(event) => info!("ACME Event: {:?}", event),
                        None => error!("Received no acme event"),
                      }
                  },
                  _ = shutdown_receiver.changed() => {
                      break;
                  }
                }
            }
        }
    }

    async fn accept_tls(
        listener: &TokioTcpListener,
        acceptor: &ItsiTlsAcceptor,
    ) -> Result<IoStream> {
        let tcp_stream = listener.accept().await?;
        match acceptor {
            ItsiTlsAcceptor::Manual(tls_acceptor) => {
                Self::to_tokio_io(Stream::TcpStream(tcp_stream), Some(tls_acceptor)).await
            }
            ItsiTlsAcceptor::Automatic(acme_acceptor, _, rustls_config) => {
                let accept_future = acme_acceptor.accept(tcp_stream.0);
                match accept_future.await {
                    Ok(None) => Err(ItsiError::Pass()),
                    Ok(Some(start_handshake)) => {
                        let tls_stream = start_handshake.into_stream(rustls_config.clone()).await?;
                        Ok(IoStream::TcpTls {
                            stream: tls_stream,
                            addr: SockAddr::Tcp(Arc::new(tcp_stream.1)),
                        })
                    }
                    Err(error) => {
                        error!(error = format!("{:?}", error));
                        Err(ItsiError::Pass())
                    }
                }
            }
        }
    }

    async fn accept_unix(listener: &TokioUnixListener) -> Result<IoStream> {
        let unix_stream = listener.accept().await?;
        Self::to_tokio_io(Stream::UnixStream(unix_stream), None).await
    }

    async fn accept_unix_tls(
        listener: &TokioUnixListener,
        acceptor: &ItsiTlsAcceptor,
    ) -> Result<IoStream> {
        let unix_stream = listener.accept().await?;
        match acceptor {
            ItsiTlsAcceptor::Manual(tls_acceptor) => {
                Self::to_tokio_io(Stream::UnixStream(unix_stream), Some(tls_acceptor)).await
            }
            ItsiTlsAcceptor::Automatic(_, _, _) => {
                error!("Automatic TLS not supported on Unix sockets");
                Err(ItsiError::UnsupportedProtocol(
                    "Automatic TLS on Unix Sockets".to_owned(),
                ))
            }
        }
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
            Listener::TcpTls((listener, acceptor)) => TokioListener::TcpTls(
                TokioTcpListener::from_std(TcpListener::try_clone(listener).unwrap()).unwrap(),
                acceptor.clone(),
            ),
            Listener::Unix(listener) => TokioListener::Unix(
                TokioUnixListener::from_std(UnixListener::try_clone(listener).unwrap()).unwrap(),
            ),
            Listener::UnixTls((listener, acceptor)) => TokioListener::UnixTls(
                TokioUnixListener::from_std(UnixListener::try_clone(listener).unwrap()).unwrap(),
                acceptor.clone(),
            ),
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
                    Listener::TcpTls((tcp_listener, bind.tls_config.unwrap()))
                }
                _ => unreachable!(),
            },
            BindAddress::UnixSocket(path) => match bind.tls_config {
                Some(tls_config) => Listener::UnixTls((connect_unix_socket(&path)?, tls_config)),
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
    socket.set_reuse_port(true).ok();
    socket.set_reuse_address(true).ok();
    socket.set_nonblocking(true).ok();
    socket.set_nodelay(true).ok();
    socket.set_recv_buffer_size(262_144).ok();
    info!("Binding to {:?}", socket_address);
    socket.bind(&socket_address.into())?;
    socket.listen(1024)?;
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
