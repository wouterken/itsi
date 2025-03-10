use std::{
    collections::VecDeque,
    os::fd::RawFd,
    sync::atomic::{AtomicUsize, Ordering},
};

use derive_more::Debug;
use itsi_rb_helpers::HeapFiber;

use mio::{event::Source, unix::SourceFd, Interest, Token};

static WAITER_TOKEN: AtomicUsize = AtomicUsize::new(1);

#[derive(Debug)]
pub struct IoWaiter {
    #[debug(skip)]
    pub fibers: VecDeque<HeapFiber>,
    fd: RawFd,
    pub token: Token,
}

/// Creates a new token for use with the scheduler.
fn new_token() -> Token {
    Token(WAITER_TOKEN.fetch_add(1, Ordering::SeqCst))
}

impl IoWaiter {
    pub fn new(fd: RawFd) -> Self {
        Self {
            fd,
            token: new_token(),
            fibers: VecDeque::new(),
        }
    }
}

impl Source for IoWaiter {
    fn register(
        &mut self,
        registry: &mio::Registry,
        token: Token,
        interests: Interest,
    ) -> std::io::Result<()> {
        SourceFd(&self.fd).register(registry, token, interests)
    }

    fn reregister(
        &mut self,
        registry: &mio::Registry,
        token: Token,
        interests: Interest,
    ) -> std::io::Result<()> {
        SourceFd(&self.fd).reregister(registry, token, interests)
    }

    fn deregister(&mut self, registry: &mio::Registry) -> std::io::Result<()> {
        SourceFd(&self.fd).deregister(registry)
    }
}
