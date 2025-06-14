use crate::prelude::*;
use crate::ruby_types::itsi_server::itsi_server_config::SocketOpts;
use crate::server::io_stream::IoStream;
use crate::server::serve_strategy::single_mode::RunningPhase;

use super::bind::{Bind, BindAddress};
use super::bind_protocol::BindProtocol;

use super::tls::ItsiTlsAcceptor;
use itsi_error::{ItsiError, Result};
use itsi_tracing::info;
use socket2::{Domain, Protocol, SockRef, Socket, Type};
use std::fmt::Display;
use std::net::{IpAddr, SocketAddr, TcpListener};
use std::os::fd::{AsRawFd, FromRawFd, RawFd};
use std::sync::Arc;
use std::{os::unix::net::UnixListener, path::PathBuf};
use tokio::net::TcpListener as TokioTcpListener;
use tokio::net::UnixListener as TokioUnixListener;
use tokio::net::{unix, TcpStream, UnixStream};
use tokio::sync::watch::Receiver;
use tokio_rustls::TlsAcceptor;
use tokio_stream::StreamExt;

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

    pub async fn spawn_acme_event_task(&self, mut shutdown_receiver: Receiver<RunningPhase>) {
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
                    Ok(None) => Err(ItsiError::Pass),
                    Ok(Some(start_handshake)) => {
                        let tls_stream = start_handshake.into_stream(rustls_config.clone()).await?;
                        Ok(IoStream::TcpTls {
                            stream: tls_stream,
                            addr: SockAddr::Tcp(Arc::new(tcp_stream.1)),
                        })
                    }
                    Err(error) => {
                        error!(error = format!("{:?}", error));
                        Err(ItsiError::Pass)
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
impl Display for Listener {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Listener::Tcp(listener) | Listener::TcpTls((listener, _)) => write!(
                f,
                "{}",
                listener
                    .local_addr()
                    .map(|addr| addr.to_string())
                    .unwrap_or_else(|_| "".to_string())
            ),

            Listener::Unix(listener) | Listener::UnixTls((listener, _)) => write!(
                f,
                "{}",
                listener
                    .local_addr()
                    .map(|addr| addr
                        .as_pathname()
                        .map(|path| path.to_str().unwrap_or("").to_owned())
                        .unwrap_or_default())
                    .unwrap_or_else(|_| "".to_string())
            ),
        }
    }
}

impl Listener {
    pub fn rebind_listener(listener: TcpListener) -> TcpListener {
        let sock = SockRef::from(&listener);
        let (reuse_address, reuse_port) = (
            sock.reuse_address().unwrap_or(true),
            sock.reuse_port().unwrap_or(true),
        );

        if !reuse_address || !reuse_port {
            return listener;
        }

        let (ip, port) = sock
            .local_addr()
            .unwrap()
            .as_socket()
            .map(|addr| (addr.ip(), addr.port()))
            .unwrap();

        let socket_opts = SocketOpts {
            reuse_address: sock.reuse_address().unwrap_or(true), // default: true
            reuse_port: sock.reuse_port().unwrap_or(false),      // default: false
            nodelay: sock.nodelay().unwrap_or(false),            // default: false
            recv_buffer_size: sock.recv_buffer_size().unwrap_or(0),
            send_buffer_size: sock.send_buffer_size().unwrap_or(0),
            listen_backlog: 1024, // cannot query – pick sane default
        };

        connect_tcp_socket(ip, port, &socket_opts).unwrap()
    }

    pub fn into_tokio_listener(self, no_rebind: bool) -> TokioListener {
        match self {
            Listener::Tcp(mut listener) => {
                if cfg!(target_os = "linux") && !no_rebind {
                    listener = Listener::rebind_listener(listener);
                }
                TokioListener::Tcp(TokioTcpListener::from_std(listener).unwrap())
            }
            Listener::TcpTls((mut listener, acceptor)) => {
                if cfg!(target_os = "linux") && !no_rebind {
                    listener = Listener::rebind_listener(listener);
                }
                TokioListener::TcpTls(
                    TokioTcpListener::from_std(listener).unwrap(),
                    acceptor.clone(),
                )
            }
            Listener::Unix(listener) => {
                TokioListener::Unix(TokioUnixListener::from_std(listener).unwrap())
            }
            Listener::UnixTls((listener, acceptor)) => TokioListener::UnixTls(
                TokioUnixListener::from_std(listener).unwrap(),
                acceptor.clone(),
            ),
        }
    }

    /// Handover information when using exec to hand over the listener to a replacement process.
    pub fn handover(&self) -> Result<(String, i32)> {
        match self {
            Listener::Tcp(listener) => {
                let addr = listener.local_addr()?;
                Ok((
                    format!("tcp://{}:{}", addr.ip().to_canonical(), addr.port()),
                    listener.as_raw_fd(),
                ))
            }
            Listener::TcpTls((listener, _)) => {
                let addr = listener.local_addr()?;
                Ok((
                    format!("tcp://{}:{}", addr.ip().to_canonical(), addr.port()),
                    listener.as_raw_fd(),
                ))
            }
            Listener::Unix(listener) => {
                let addr = listener.local_addr()?;
                Ok((
                    format!("unix://{}", addr.as_pathname().unwrap().to_str().unwrap()),
                    listener.as_raw_fd(),
                ))
            }
            Listener::UnixTls((listener, _)) => {
                let addr = listener.local_addr()?;
                Ok((
                    format!("unix://{}", addr.as_pathname().unwrap().to_str().unwrap()),
                    listener.as_raw_fd(),
                ))
            }
        }
    }

    pub fn inherit_fd(bind: Bind, fd: RawFd, socket_opts: &SocketOpts) -> Result<Self> {
        let bound = match bind.address {
            BindAddress::Ip(_) => match bind.protocol {
                BindProtocol::Http => Listener::Tcp(revive_tcp_socket(fd, socket_opts)?),
                BindProtocol::Https => {
                    let tcp_listener = revive_tcp_socket(fd, socket_opts)?;
                    Listener::TcpTls((
                        tcp_listener,
                        bind.tls_config.unwrap().build_acceptor().unwrap(),
                    ))
                }
                _ => unreachable!(),
            },
            BindAddress::UnixSocket(_) => match bind.tls_config {
                Some(tls_config) => Listener::UnixTls((
                    revive_unix_socket(fd, socket_opts)?,
                    tls_config.build_acceptor().unwrap(),
                )),
                None => Listener::Unix(revive_unix_socket(fd, socket_opts)?),
            },
        };
        Ok(bound)
    }
}

impl Listener {
    pub fn build(bind: Bind, socket_opts: &SocketOpts) -> Result<Self> {
        let bound = match bind.address {
            BindAddress::Ip(addr) => match bind.protocol {
                BindProtocol::Http => {
                    Listener::Tcp(connect_tcp_socket(addr, bind.port.unwrap(), socket_opts)?)
                }
                BindProtocol::Https => {
                    let tcp_listener = connect_tcp_socket(addr, bind.port.unwrap(), socket_opts)?;
                    Listener::TcpTls((
                        tcp_listener,
                        bind.tls_config.unwrap().build_acceptor().unwrap(),
                    ))
                }
                _ => unreachable!(),
            },
            BindAddress::UnixSocket(path) => match bind.tls_config {
                Some(tls_config) => Listener::UnixTls((
                    connect_unix_socket(&path, socket_opts)?,
                    tls_config.build_acceptor().unwrap(),
                )),
                None => Listener::Unix(connect_unix_socket(&path, socket_opts)?),
            },
        };
        Ok(bound)
    }
}

fn revive_tcp_socket(fd: RawFd, socket_opts: &SocketOpts) -> Result<TcpListener> {
    let socket = unsafe { Socket::from_raw_fd(fd) };
    socket.set_reuse_port(socket_opts.reuse_port).ok();
    socket.set_reuse_address(socket_opts.reuse_address).ok();
    socket.set_nonblocking(true).ok();
    socket.set_nodelay(socket_opts.nodelay).ok();
    socket
        .set_recv_buffer_size(socket_opts.recv_buffer_size)
        .ok();
    socket.set_cloexec(true)?;
    socket.listen(socket_opts.listen_backlog as i32)?;
    Ok(socket.into())
}

fn revive_unix_socket(fd: RawFd, socket_opts: &SocketOpts) -> Result<UnixListener> {
    let socket = unsafe { Socket::from_raw_fd(fd) };
    socket.set_nonblocking(true).ok();
    socket.listen(socket_opts.listen_backlog as i32)?;
    socket.set_cloexec(true)?;

    Ok(socket.into())
}

fn connect_tcp_socket(addr: IpAddr, port: u16, socket_opts: &SocketOpts) -> Result<TcpListener> {
    let domain = match addr {
        IpAddr::V4(_) => Domain::IPV4,
        IpAddr::V6(_) => Domain::IPV6,
    };
    let socket = Socket::new(domain, Type::STREAM, Some(Protocol::TCP))?;
    let socket_address: SocketAddr = SocketAddr::new(addr, port);
    socket.set_reuse_address(socket_opts.reuse_address).ok();
    socket.set_reuse_port(socket_opts.reuse_port).ok();
    socket.set_nonblocking(true).ok();
    socket.set_nodelay(socket_opts.nodelay).ok();
    socket
        .set_send_buffer_size(socket_opts.send_buffer_size)
        .ok();
    socket
        .set_recv_buffer_size(socket_opts.recv_buffer_size)
        .ok();
    socket.set_only_v6(false).ok();
    if let Err(e) = socket.bind(&socket_address.into()) {
        error!("Failed to bind socket: {}", e);
    };
    if let Err(e) = socket.listen(socket_opts.listen_backlog as i32) {
        error!("Failed to listen on socket: {}", e);
    };
    Ok(socket.into())
}

fn connect_unix_socket(path: &PathBuf, socket_opts: &SocketOpts) -> Result<UnixListener> {
    let _ = std::fs::remove_file(path);
    let socket = Socket::new(Domain::UNIX, Type::STREAM, None)?;
    socket.set_nonblocking(true).ok();

    let socket_address = socket2::SockAddr::unix(path)?;

    if let Err(e) = socket.bind(&socket_address) {
        error!("Failed to bind socket: {}", e);
    };
    if let Err(e) = socket.listen(socket_opts.listen_backlog as i32) {
        error!("Failed to listen on socket: {}", e);
    };
    Ok(socket.into())
}
