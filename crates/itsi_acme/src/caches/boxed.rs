use crate::{AccountCache, CertCache};
use async_trait::async_trait;
use std::fmt::Debug;

pub struct BoxedErrCache<T: Send + Sync> {
    inner: T,
}

impl<T: Send + Sync> BoxedErrCache<T> {
    pub fn new(inner: T) -> Self {
        Self { inner }
    }
    pub fn into_inner(self) -> T {
        self.inner
    }
}

fn box_err(e: impl Debug + 'static) -> Box<dyn Debug> {
    Box::new(e)
}

#[async_trait]
impl<T: CertCache> CertCache for BoxedErrCache<T>
where
    <T as CertCache>::EC: 'static,
{
    type EC = Box<dyn Debug>;
    async fn load_cert(
        &self,
        domains: &[String],
        directory_url: &str,
    ) -> Result<Option<Vec<u8>>, Self::EC> {
        self.inner
            .load_cert(domains, directory_url)
            .await
            .map_err(box_err)
    }

    async fn store_cert(
        &self,
        domains: &[String],
        directory_url: &str,
        cert: &[u8],
    ) -> Result<(), Self::EC> {
        self.inner
            .store_cert(domains, directory_url, cert)
            .await
            .map_err(box_err)
    }
}

#[async_trait]
impl<T: AccountCache> AccountCache for BoxedErrCache<T>
where
    <T as AccountCache>::EA: 'static,
{
    type EA = Box<dyn Debug>;
    async fn load_account(
        &self,
        contact: &[String],
        directory_url: &str,
    ) -> Result<Option<Vec<u8>>, Self::EA> {
        self.inner
            .load_account(contact, directory_url)
            .await
            .map_err(box_err)
    }

    async fn store_account(
        &self,
        contact: &[String],
        directory_url: &str,
        account: &[u8],
    ) -> Result<(), Self::EA> {
        self.inner
            .store_account(contact, directory_url, account)
            .await
            .map_err(box_err)
    }
}
