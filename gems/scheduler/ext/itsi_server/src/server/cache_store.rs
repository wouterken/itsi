use async_trait::async_trait;
use redis::aio::ConnectionManager;
use redis::{Client, RedisError, Script};
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug)]
pub enum CacheError {
    RedisError(RedisError),
    // Other error variants as needed.
}

/// A general-purpose cache trait with an atomic “increment with timeout” operation.
#[async_trait]
pub trait CacheStore: Send + Sync {
    /// Increments the counter associated with `key` and sets (or extends) its expiration.
    /// Returns the new counter value.
    async fn increment(&self, key: &str, timeout: Duration) -> Result<u64, CacheError>;
}

/// A Redis-backed cache store using an async connection manager.
/// This uses a TLS-enabled connection when the URL is prefixed with "rediss://".
#[derive(Clone)]
pub struct RedisCacheStore {
    connection: Arc<ConnectionManager>,
}

impl RedisCacheStore {
    /// Constructs a new RedisCacheStore.
    ///
    /// Use a connection URL like "rediss://host:port" to enable TLS (with rustls under the hood).
    /// This constructor is async because it sets up the connection manager.
    pub async fn new(connection_url: &str) -> Result<Self, CacheError> {
        let client = Client::open(connection_url).map_err(CacheError::RedisError)?;
        let connection_manager = ConnectionManager::new(client)
            .await
            .map_err(CacheError::RedisError)?;
        Ok(Self {
            connection: Arc::new(connection_manager),
        })
    }
}

#[async_trait]
impl CacheStore for RedisCacheStore {
    async fn increment(&self, key: &str, timeout: Duration) -> Result<u64, CacheError> {
        let timeout_secs = timeout.as_secs();
        // Lua script to:
        // 1. INCR the key.
        // 2. If the key doesn't have a TTL, set it.
        let script = r#"
            local current = redis.call('INCR', KEYS[1])
            if redis.call('TTL', KEYS[1]) < 0 then
                redis.call('EXPIRE', KEYS[1], ARGV[1])
            end
            return current
        "#;
        let script = Script::new(script);
        // The ConnectionManager is cloneable and can be used concurrently.
        let mut connection = (*self.connection).clone();
        let value: i64 = script
            .key(key)
            .arg(timeout_secs)
            .invoke_async(&mut connection)
            .await
            .map_err(CacheError::RedisError)?;
        Ok(value as u64)
    }
}
