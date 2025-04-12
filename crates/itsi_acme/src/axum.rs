use crate::{AcmeAccept, AcmeAcceptor};
use rustls::ServerConfig;
use std::future::Future;
use std::io;
use std::io::ErrorKind;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_rustls::Accept;

#[derive(Clone)]
pub struct AxumAcceptor {
    acme_acceptor: AcmeAcceptor,
    config: Arc<ServerConfig>,
}

impl AxumAcceptor {
    pub fn new(acme_acceptor: AcmeAcceptor, config: Arc<ServerConfig>) -> Self {
        Self {
            acme_acceptor,
            config,
        }
    }
}

impl<I: AsyncRead + AsyncWrite + Unpin + Send + 'static, S: Send + 'static>
    axum_server::accept::Accept<I, S> for AxumAcceptor
{
    type Stream = tokio_rustls::server::TlsStream<I>;
    type Service = S;
    type Future = AxumAccept<I, S>;

    fn accept(&self, stream: I, service: S) -> Self::Future {
        let acme_accept = self.acme_acceptor.accept(stream);
        Self::Future {
            config: self.config.clone(),
            acme_accept,
            tls_accept: None,
            service: Some(service),
        }
    }
}

pub struct AxumAccept<I: AsyncRead + AsyncWrite + Unpin + Send + 'static, S: Send + 'static> {
    config: Arc<ServerConfig>,
    acme_accept: AcmeAccept<I>,
    tls_accept: Option<Accept<I>>,
    service: Option<S>,
}

impl<I: AsyncRead + AsyncWrite + Unpin + Send + 'static, S: Send + 'static> Unpin
    for AxumAccept<I, S>
{
}

impl<I: AsyncRead + AsyncWrite + Unpin + Send + 'static, S: Send + 'static> Future
    for AxumAccept<I, S>
{
    type Output = io::Result<(tokio_rustls::server::TlsStream<I>, S)>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            if let Some(tls_accept) = &mut self.tls_accept {
                return match Pin::new(&mut *tls_accept).poll(cx) {
                    Poll::Ready(Ok(tls)) => Poll::Ready(Ok((tls, self.service.take().unwrap()))),
                    Poll::Ready(Err(err)) => Poll::Ready(Err(err)),
                    Poll::Pending => Poll::Pending,
                };
            }
            return match Pin::new(&mut self.acme_accept).poll(cx) {
                Poll::Ready(Ok(Some(start_handshake))) => {
                    let config = self.config.clone();
                    self.tls_accept = Some(start_handshake.into_stream(config));
                    continue;
                }
                Poll::Ready(Ok(None)) => Poll::Ready(Err(io::Error::new(
                    ErrorKind::Other,
                    "TLS-ALPN-01 validation request",
                ))),
                Poll::Ready(Err(err)) => Poll::Ready(Err(err)),
                Poll::Pending => Poll::Pending,
            };
        }
    }
}
