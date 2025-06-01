use bytes::{Bytes, BytesMut};
use futures::Stream;
use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::sync::mpsc::Receiver;
use tokio::sync::watch;
use tokio::time::{sleep, Sleep};

use super::serve_strategy::single_mode::RunningPhase;

#[derive(Debug)]
pub struct FrameStream {
    receiver: Receiver<Bytes>,
    shutdown_rx: watch::Receiver<RunningPhase>,
    drained: bool,
}

impl FrameStream {
    pub fn new(receiver: Receiver<Bytes>, shutdown_rx: watch::Receiver<RunningPhase>) -> Self {
        Self {
            receiver,
            shutdown_rx,
            drained: false,
        }
    }
}

impl Stream for FrameStream {
    type Item = Result<Bytes, Infallible>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        if this.drained {
            return Poll::Ready(None);
        }

        match Pin::new(&mut this.receiver).poll_recv(cx) {
            Poll::Ready(Some(bytes)) => Poll::Ready(Some(Ok(bytes))),
            Poll::Ready(None) => {
                this.drained = true;
                Poll::Ready(None)
            }
            Poll::Pending => {
                if this.shutdown_rx.has_changed().unwrap_or(false)
                    && *this.shutdown_rx.borrow() == RunningPhase::ShutdownPending
                {
                    while let Ok(bytes) = this.receiver.try_recv() {
                        return Poll::Ready(Some(Ok(bytes)));
                    }
                    this.drained = true;
                    return Poll::Ready(None);
                }

                Poll::Pending
            }
        }
    }
}

/// BufferedStream wraps a stream of Bytes and coalesces chunks into a larger buffer,
/// flushing either after `max_flush_bytes` is reached or `max_flush_interval` elapses.
pub struct BufferedStream<S> {
    inner: S,
    buffer: BytesMut,
    max_flush_bytes: usize,
    max_flush_interval: Duration,
    flush_deadline: Option<Pin<Box<Sleep>>>,
}

impl<S> BufferedStream<S> {
    pub fn new(inner: S, max_flush_bytes: usize, max_flush_interval: Duration) -> Self {
        Self {
            inner,
            buffer: BytesMut::with_capacity(max_flush_bytes),
            max_flush_bytes,
            max_flush_interval,
            flush_deadline: None,
        }
    }
}

impl<S> Stream for BufferedStream<S>
where
    S: Stream<Item = Result<Bytes, Infallible>> + Unpin,
{
    type Item = Result<Bytes, Infallible>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        loop {
            // Flush on timer if needed
            if let Some(deadline) = &mut this.flush_deadline {
                if Pin::new(deadline).poll(cx).is_ready() && !this.buffer.is_empty() {
                    let flushed = this.buffer.split().freeze();
                    this.flush_deadline = None;
                    return Poll::Ready(Some(Ok(flushed)));
                }
            }

            match Pin::new(&mut this.inner).poll_next(cx) {
                Poll::Ready(Some(Ok(bytes))) => {
                    this.buffer.extend_from_slice(&bytes);

                    if bytes.is_empty() || this.buffer.len() >= this.max_flush_bytes {
                        let flushed = this.buffer.split().freeze();
                        this.flush_deadline = None;
                        return Poll::Ready(Some(Ok(flushed)));
                    }

                    if this.flush_deadline.is_none() {
                        this.flush_deadline = Some(Box::pin(sleep(this.max_flush_interval)));
                    }
                }
                Poll::Ready(None) => {
                    if this.buffer.is_empty() {
                        return Poll::Ready(None);
                    } else {
                        let flushed = this.buffer.split().freeze();
                        this.flush_deadline = None;
                        return Poll::Ready(Some(Ok(flushed)));
                    }
                }
                Poll::Pending => {
                    if let Some(deadline) = &mut this.flush_deadline {
                        let deadline = deadline.as_mut();
                        if deadline.poll(cx).is_ready() && !this.buffer.is_empty() {
                            let flushed = this.buffer.split().freeze();
                            this.flush_deadline = None;
                            return Poll::Ready(Some(Ok(flushed)));
                        }
                    }
                    return Poll::Pending;
                }
            }
        }
    }
}
