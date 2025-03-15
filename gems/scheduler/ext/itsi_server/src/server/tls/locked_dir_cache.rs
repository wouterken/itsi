use async_trait::async_trait;
use fs2::FileExt;
use parking_lot::Mutex;
use std::fs::{self, OpenOptions};
use std::io::Error as IoError;
use std::path::{Path, PathBuf};
use tokio_rustls_acme::caches::DirCache;
use tokio_rustls_acme::{AccountCache, CertCache};

use crate::env::ITSI_ACME_LOCK_FILE_NAME;

/// A wrapper around DirCache that locks a file before writing cert/account data.
pub struct LockedDirCache<P: AsRef<Path> + Send + Sync> {
    inner: DirCache<P>,
    lock_path: PathBuf,
    current_lock: Mutex<Option<std::fs::File>>,
}

impl<P: AsRef<Path> + Send + Sync> LockedDirCache<P> {
    pub fn new(dir: P) -> Self {
        let dir_path = dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&dir_path).unwrap();
        let lock_path = dir_path.join(&*ITSI_ACME_LOCK_FILE_NAME);
        Self::touch_file(&lock_path).expect("Failed to create lock file");

        Self {
            inner: DirCache::new(dir),
            lock_path,
            current_lock: Mutex::new(None),
        }
    }

    fn touch_file(path: &PathBuf) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)?;
        Ok(())
    }

    fn lock_exclusive(&self) -> Result<(), IoError> {
        if self.current_lock.lock().is_some() {
            return Ok(());
        }

        if let Some(parent) = self.lock_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let lockfile = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&self.lock_path)?;
        lockfile.lock_exclusive()?;
        *self.current_lock.lock() = Some(lockfile);
        Ok(())
    }

    fn unlock(&self) -> Result<(), IoError> {
        self.current_lock.lock().take();
        Ok(())
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
        self.lock_exclusive()?;
        let result = self.inner.load_cert(domains, directory_url).await;

        if let Ok(Some(_)) = result {
            self.unlock()?;
        }

        result
    }

    async fn store_cert(
        &self,
        domains: &[String],
        directory_url: &str,
        cert: &[u8],
    ) -> Result<(), Self::EC> {
        // Acquire the lock before storing
        self.lock_exclusive()?;

        // Perform the store operation
        let result = self.inner.store_cert(domains, directory_url, cert).await;

        if let Ok(()) = result {
            self.unlock()?;
        }
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
        self.lock_exclusive()?;
        self.inner.load_account(contact, directory_url).await
    }

    async fn store_account(
        &self,
        contact: &[String],
        directory_url: &str,
        account: &[u8],
    ) -> Result<(), Self::EA> {
        self.lock_exclusive()?;

        self.inner
            .store_account(contact, directory_url, account)
            .await
    }
}
