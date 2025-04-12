use crate::{AccountCache, CertCache};
use async_trait::async_trait;
use std::convert::Infallible;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::sync::atomic::AtomicPtr;

/// No-op cache, which does nothing.
/// ```rust
/// # use tokio_rustls_acme::caches::NoCache;
/// # type EC = std::io::Error;
/// # type EA = EC;
/// let no_cache = NoCache::<EC, EA>::new();
/// ```
#[derive(Copy, Clone)]
pub struct NoCache<EC: Debug = Infallible, EA: Debug = Infallible> {
    _cert_error: PhantomData<AtomicPtr<Box<EC>>>,
    _account_error: PhantomData<AtomicPtr<Box<EA>>>,
}

impl<EC: Debug, EA: Debug> Default for NoCache<EC, EA> {
    fn default() -> Self {
        Self {
            _cert_error: Default::default(),
            _account_error: Default::default(),
        }
    }
}

impl<EC: Debug, EA: Debug> NoCache<EC, EA> {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl<EC: Debug, EA: Debug> CertCache for NoCache<EC, EA> {
    type EC = EC;
    async fn load_cert(
        &self,
        _domains: &[String],
        _directory_url: &str,
    ) -> Result<Option<Vec<u8>>, Self::EC> {
        log::info!("no cert cache configured, could not load certificate");
        Ok(None)
    }
    async fn store_cert(
        &self,
        _domains: &[String],
        _directory_url: &str,
        _cert: &[u8],
    ) -> Result<(), Self::EC> {
        log::info!("no cert cache configured, could not store certificate");
        Ok(())
    }
}

#[async_trait]
impl<EC: Debug, EA: Debug> AccountCache for NoCache<EC, EA> {
    type EA = EA;
    async fn load_account(
        &self,
        _contact: &[String],
        _directory_url: &str,
    ) -> Result<Option<Vec<u8>>, Self::EA> {
        log::info!("no account cache configured, could not load account");
        Ok(None)
    }
    async fn store_account(
        &self,
        _contact: &[String],
        _directory_url: &str,
        _account: &[u8],
    ) -> Result<(), Self::EA> {
        log::info!("no account cache configured, could not store account");
        Ok(())
    }
}
