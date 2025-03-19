use bytes::Bytes;
use http_body_util::{combinators::BoxBody, Full};
use moka::sync::Cache;
use std::{
    convert::Infallible,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant, SystemTime},
};
use tokio::{fs::File, io::AsyncReadExt};

#[derive(Clone)]
struct CachedFile {
    content: Arc<Bytes>,
    last_modified: SystemTime,
    last_checked: Instant,
}

pub struct StaticFileCache {
    pub root_dir: PathBuf,
    cache: Cache<PathBuf, CachedFile>,
    pub max_file_size: u64,
    pub recheck_interval: Duration,
}

impl StaticFileCache {
    pub fn new<P: Into<PathBuf>>(
        root_dir: P,
        max_entries: u64,
        max_file_size: u64,
        recheck_interval: Duration,
    ) -> Self {
        let cache = Cache::builder().max_capacity(max_entries).build();

        StaticFileCache {
            root_dir: root_dir.into(),
            cache,
            max_file_size,
            recheck_interval,
        }
    }

    pub async fn serve_static_file(&self, path: &Path) -> Option<BoxBody<Bytes, Infallible>> {
        let full_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.root_dir.join(path)
        };

        if let Some(entry) = self.cache.get(&full_path) {
            if entry.last_checked.elapsed() < self.recheck_interval {
                return Some(build_ok_body(entry.content.clone()));
            } else if let Ok(metadata) = tokio::fs::metadata(&full_path).await {
                if let Ok(mod_time) = metadata.modified() {
                    if mod_time == entry.last_modified {
                        let updated = CachedFile {
                            content: entry.content.clone(),
                            last_modified: entry.last_modified,
                            last_checked: Instant::now(),
                        };
                        self.cache.insert(full_path.clone(), updated.clone());
                        return Some(build_ok_body(updated.content));
                    }
                }
            }
        }

        match tokio::fs::metadata(&full_path).await {
            Ok(metadata) => {
                if metadata.len() > self.max_file_size {
                    return self.stream_file(&full_path).await;
                }

                match read_entire_file(&full_path).await {
                    Ok((content, last_modified)) => {
                        let cached_file = CachedFile {
                            content: Arc::new(content),
                            last_modified,
                            last_checked: Instant::now(),
                        };
                        self.cache.insert(full_path.clone(), cached_file.clone());
                        Some(build_ok_body(cached_file.content))
                    }
                    Err(_) => None,
                }
            }
            Err(_) => None,
        }
    }

    pub async fn serve_static_dir(&self, uri_path: &str) -> Option<BoxBody<Bytes, Infallible>> {
        let rel = Path::new(uri_path.trim_start_matches('/'));
        self.serve_static_file(rel).await
    }

    async fn stream_file(&self, path: &Path) -> Option<BoxBody<Bytes, Infallible>> {
        use futures::TryStreamExt;
        use http_body_util::StreamBody;
        use hyper::body::Frame;
        use tokio_util::io::ReaderStream;

        match File::open(path).await {
            Ok(file) => {
                let stream = ReaderStream::new(file)
                    .map_ok(Frame::data)
                    .map_err(|_| -> Infallible { unreachable!("We handle IO errors above") });
                Some(BoxBody::new(StreamBody::new(stream)))
            }
            Err(_) => None,
        }
    }
}

async fn read_entire_file(path: &Path) -> std::io::Result<(Bytes, SystemTime)> {
    let metadata = tokio::fs::metadata(path).await?;
    let last_modified = metadata.modified()?;
    let mut file = File::open(path).await?;
    let mut buf = Vec::with_capacity(metadata.len().try_into().unwrap_or(4096));
    file.read_to_end(&mut buf).await?;
    Ok((Bytes::from(buf), last_modified))
}

fn build_ok_body(bytes: Arc<Bytes>) -> BoxBody<Bytes, Infallible> {
    BoxBody::new(Full::new(bytes.as_ref().clone()))
}
