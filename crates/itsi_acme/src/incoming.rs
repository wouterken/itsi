use crate::acceptor::{AcmeAccept, AcmeAcceptor};
use crate::AcmeState;
use futures::stream::{FusedStream, FuturesUnordered};
use futures::Stream;
use rustls::ServerConfig;
use std::fmt::Debug;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_rustls::{server::TlsStream, Accept};

pub struct Incoming<
    TCP: AsyncRead + AsyncWrite + Unpin,
    ETCP,
    ITCP: Stream<Item = Result<TCP, ETCP>> + Unpin,
    EC: Debug + 'static,
    EA: Debug + 'static,
> {
    state: AcmeState<EC, EA>,
    acceptor: AcmeAcceptor,
    rustls_config: Arc<ServerConfig>,
    tcp_incoming: Option<ITCP>,
    acme_accepting: FuturesUnordered<AcmeAccept<TCP>>,
    tls_accepting: FuturesUnordered<Accept<TCP>>,
}

impl<
        TCP: AsyncRead + AsyncWrite + Unpin,
        ETCP,
        ITCP: Stream<Item = Result<TCP, ETCP>> + Unpin,
        EC: Debug + 'static,
        EA: Debug + 'static,
    > Unpin for Incoming<TCP, ETCP, ITCP, EC, EA>
{
}

impl<
        TCP: AsyncRead + AsyncWrite + Unpin,
        ETCP,
        ITCP: Stream<Item = Result<TCP, ETCP>> + Unpin,
        EC: Debug + 'static,
        EA: Debug + 'static,
    > Incoming<TCP, ETCP, ITCP, EC, EA>
{
    pub fn new(
        tcp_incoming: ITCP,
        state: AcmeState<EC, EA>,
        acceptor: AcmeAcceptor,
        alpn_protocols: Vec<Vec<u8>>,
    ) -> Self {
        let mut config = ServerConfig::builder()
            .with_no_client_auth()
            .with_cert_resolver(state.resolver());
        config.alpn_protocols = alpn_protocols;
        Self {
            state,
            acceptor,
            rustls_config: Arc::new(config),
            tcp_incoming: Some(tcp_incoming),
            acme_accepting: FuturesUnordered::new(),
            tls_accepting: FuturesUnordered::new(),
        }
    }
}

impl<
        TCP: AsyncRead + AsyncWrite + Unpin,
        ETCP,
        ITCP: Stream<Item = Result<TCP, ETCP>> + Unpin,
        EC: Debug + 'static,
        EA: Debug + 'static,
    > Stream for Incoming<TCP, ETCP, ITCP, EC, EA>
{
    type Item = Result<TlsStream<TCP>, ETCP>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            match Pin::new(&mut self.state).poll_next(cx) {
                Poll::Ready(Some(event)) => {
                    match event {
                        Ok(ok) => log::info!("event: {:?}", ok),
                        Err(err) => log::error!("event: {:?}", err),
                    }
                    continue;
                }
                Poll::Ready(None) => unreachable!(),
                Poll::Pending => {}
            }
            match Pin::new(&mut self.acme_accepting).poll_next(cx) {
                Poll::Ready(Some(Ok(Some(tls)))) => self
                    .tls_accepting
                    .push(tls.into_stream(self.rustls_config.clone())),
                Poll::Ready(Some(Ok(None))) => {
                    log::info!("received TLS-ALPN-01 validation request");
                    continue;
                }
                Poll::Ready(Some(Err(err))) => {
                    log::error!("tls accept failed, {:?}", err);
                    continue;
                }
                Poll::Ready(None) | Poll::Pending => {}
            }
            match Pin::new(&mut self.tls_accepting).poll_next(cx) {
                Poll::Ready(Some(Ok(tls))) => return Poll::Ready(Some(Ok(tls))),
                Poll::Ready(Some(Err(err))) => {
                    log::error!("tls accept failed, {:?}", err);
                    continue;
                }
                Poll::Ready(None) | Poll::Pending => {}
            }
            let tcp_incoming = match &mut self.tcp_incoming {
                Some(tcp_incoming) => tcp_incoming,
                None => match self.is_terminated() {
                    true => return Poll::Ready(None),
                    false => return Poll::Pending,
                },
            };
            match Pin::new(tcp_incoming).poll_next(cx) {
                Poll::Ready(Some(Ok(tcp))) => self.acme_accepting.push(self.acceptor.accept(tcp)),
                Poll::Ready(Some(Err(err))) => return Poll::Ready(Some(Err(err))),
                Poll::Ready(None) => drop(self.tcp_incoming.as_mut()),
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

impl<
        TCP: AsyncRead + AsyncWrite + Unpin,
        ETCP,
        ITCP: Stream<Item = Result<TCP, ETCP>> + Unpin,
        EC: Debug + 'static,
        EA: Debug + 'static,
    > FusedStream for Incoming<TCP, ETCP, ITCP, EC, EA>
{
    fn is_terminated(&self) -> bool {
        self.tcp_incoming.is_none()
            && self.acme_accepting.is_terminated()
            && self.tls_accepting.is_terminated()
    }
}
