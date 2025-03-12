use derive_more::Debug;
use mio::{event::Source, unix::SourceFd, Interest, Token};
use std::os::fd::RawFd;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IoWaiter {
    pub fd: RawFd,
    pub readiness: i16,
    pub token: Token,
}

impl IoWaiter {
    pub fn new(fd: RawFd, readiness: i16, token: Token) -> Self {
        Self {
            fd,
            readiness,
            token,
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
