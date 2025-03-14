use async_trait::async_trait;
use fs2::FileExt; // for lock_exclusive, unlock
use std::fs::OpenOptions;
use std::io::Error as IoError;
use std::path::{Path, PathBuf};
use tokio_rustls_acme::caches::DirCache;
use tokio_rustls_acme::{AccountCache, CertCache};

/// A wrapper around DirCache that locks a file before writing cert/account data.
pub struct LockedDirCache<P: AsRef<Path> + Send + Sync> {
    inner: DirCache<P>,
    lock_path: PathBuf,
}

impl<P: AsRef<Path> + Send + Sync> LockedDirCache<P> {
    pub fn new(dir: P) -> Self {
        let dir_path = dir.as_ref().to_path_buf();
        let lock_path = dir_path.join(".acme.lock");
        Self {
            inner: DirCache::new(dir),
            lock_path,
        }
    }

    fn lock_exclusive(&self) -> Result<std::fs::File, IoError> {
        let lockfile = OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&self.lock_path)?;
        lockfile.lock_exclusive()?;
        Ok(lockfile)
    }
}

#[async_trait]
impl<P: AsRef<Path> + Send + Sync> CertCache for LockedDirCache<P> {
    type EC = IoError;

    async fn load_cert(
        &self,
        domains: &[String],
        directory_url: &str,
    ) -> Result<Option<Vec<u8>>, Self::EC> {
        // Just delegate to the inner DirCache
        self.inner.load_cert(domains, directory_url).await
    }

    async fn store_cert(
        &self,
        domains: &[String],
        directory_url: &str,
        cert: &[u8],
    ) -> Result<(), Self::EC> {
        // Acquire the lock before storing
        let lockfile = self.lock_exclusive()?;

        // Perform the store operation
        let result = self.inner.store_cert(domains, directory_url, cert).await;

        // Unlock and return
        let _ = fs2::FileExt::unlock(&lockfile);
        result
    }
}

#[async_trait]
impl<P: AsRef<Path> + Send + Sync> AccountCache for LockedDirCache<P> {
    type EA = IoError;

    async fn load_account(
        &self,
        contact: &[String],
        directory_url: &str,
    ) -> Result<Option<Vec<u8>>, Self::EA> {
        self.inner.load_account(contact, directory_url).await
    }

    async fn store_account(
        &self,
        contact: &[String],
        directory_url: &str,
        account: &[u8],
    ) -> Result<(), Self::EA> {
        let lockfile = self.lock_exclusive()?;

        let result = self
            .inner
            .store_account(contact, directory_url, account)
            .await;

        let _ = fs2::FileExt::unlock(&lockfile);
        result
    }
}
