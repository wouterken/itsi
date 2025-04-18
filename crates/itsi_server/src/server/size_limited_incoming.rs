use bytes::Buf;
use hyper::body::Body;
use hyper::body::Frame;
use hyper::body::SizeHint;
use std::error::Error;
use std::fmt;
use std::ops::Deref;
use std::pin::Pin;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::task::Context;
use std::task::Poll;
use tracing::debug;

/// Custom error to indicate that the maximum body size was exceeded.
#[derive(Debug)]
pub struct MaxBodySizeReached;
impl fmt::Display for MaxBodySizeReached {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Maximum body size reached")
    }
}

impl Error for MaxBodySizeReached {}

#[derive(Debug)]
pub struct SizeLimitedIncoming<B> {
    pub inner: B,
    pub limit: AtomicUsize,
    current: usize,
}

impl<B> Deref for SizeLimitedIncoming<B> {
    type Target = B;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<B> SizeLimitedIncoming<B> {
    pub fn new(inner: B) -> Self {
        Self {
            inner,
            limit: AtomicUsize::new(usize::MAX),
            current: 0,
        }
    }
}

impl<B> Body for SizeLimitedIncoming<B>
where
    B: Body + Unpin,
    B::Data: Buf,
    // Ensure that the inner error converts into our boxed error type.
    B::Error: Into<Box<dyn Error + Send + Sync>>,
{
    type Data = B::Data;
    type Error = Box<dyn Error + Send + Sync>;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        // Pin the inner body.
        let inner = Pin::new(&mut self.inner);
        match inner.poll_frame(cx) {
            Poll::Ready(Some(Ok(frame))) => {
                // Use public methods since we cannot match on the private enum.
                if frame.is_data() {
                    match frame.into_data() {
                        Ok(data) => {
                            let len = data.remaining();
                            self.current += len;
                            debug!(
                              target: "option::max_body",
                              "current: {}, limit: {}",
                              self.current, self.limit.load(Ordering::Relaxed)
                            );
                            if self.current > self.limit.load(Ordering::Relaxed) {
                                Poll::Ready(Some(Err(Box::new(MaxBodySizeReached))))
                            } else {
                                Poll::Ready(Some(Ok(Frame::data(data))))
                            }
                        }
                        // Should not occur if is_data() was true, but pass through if it does.
                        Err(frame) => Poll::Ready(Some(Ok(frame))),
                    }
                } else {
                    // For non-data frames (e.g. trailers), just pass them along.
                    Poll::Ready(Some(Ok(frame)))
                }
            }
            Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(e.into()))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }

    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }

    fn size_hint(&self) -> SizeHint {
        self.inner.size_hint()
    }
}
