use async_trait::async_trait;
use rand::Rng;
use redis::aio::ConnectionManager;
use redis::{Client, RedisError, Script};
use serde::Deserialize;
use std::any::Any;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::{Mutex as AsyncMutex, RwLock};
use tokio::time::timeout;
use url::Url;

#[derive(Debug)]
pub enum RateLimitError {
    RedisError(RedisError),
    RateLimitExceeded { limit: u64, count: u64 },
    LockError,
    ConnectionTimeout,
    // Other error variants as needed.
}

impl From<RedisError> for RateLimitError {
    fn from(err: RedisError) -> Self {
        RateLimitError::RedisError(err)
    }
}

impl std::fmt::Display for RateLimitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RateLimitError::RedisError(e) => write!(f, "Redis error: {}", e),
            RateLimitError::RateLimitExceeded { limit, count } => {
                write!(f, "Rate limit exceeded: {}/{}", count, limit)
            }
            RateLimitError::LockError => write!(f, "Failed to acquire lock"),
            RateLimitError::ConnectionTimeout => write!(f, "Connection timeout"),
        }
    }
}

/// A RateLimiter trait for limiting HTTP requests
#[async_trait]
pub trait RateLimiter: Send + Sync + std::fmt::Debug {
    /// Increments the counter associated with `key` and sets its expiration.
    /// Returns the new counter value.
    ///
    /// If the operation fails, returns Ok(0) to fail open.
    async fn increment(&self, key: &str, timeout: Duration) -> Result<u64, RateLimitError>;

    /// Checks if the rate limit is exceeded for the given key.
    /// Returns Ok(current_count) if not exceeded, or Err(RateLimitExceeded) if exceeded.
    ///
    /// If there's an error (like connectivity issues), this will always return Ok
    /// to allow the request through (fail open).
    async fn check_limit(
        &self,
        key: &str,
        limit: u64,
        timeout: Duration,
    ) -> Result<u64, RateLimitError>;

    /// Returns self as Any for downcasting
    fn as_any(&self) -> &dyn Any;
}

/// A Redis-backed rate limiter using an async connection manager.
/// This uses a TLS-enabled connection when the URL is prefixed with "rediss://".
#[derive(Clone)]
pub struct RedisRateLimiter {
    connection: Arc<ConnectionManager>,
    increment_script: Script,
}

impl std::fmt::Debug for RedisRateLimiter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RedisRateLimiter").finish()
    }
}

impl RedisRateLimiter {
    /// Constructs a new RedisRateLimiter with a timeout.
    ///
    /// Use a connection URL like:
    /// - Standard: "redis://host:port/db"
    /// - With auth: "redis://:password@host:port/db"
    /// - With TLS: "rediss://host:port/db"
    /// - With TLS and auth: "rediss://:password@host:port/db"
    pub async fn new(connection_url: &str) -> Result<Self, RateLimitError> {
        // Set a reasonable timeout for connection attempts (5 seconds)
        const CONNECTION_TIMEOUT: Duration = Duration::from_secs(5);

        // Parse URL to extract auth information if provided
        let url_result = Url::parse(connection_url);
        if let Err(e) = url_result {
            tracing::error!("Invalid Redis URL format: {}", e);
            return Err(RateLimitError::RedisError(RedisError::from((
                redis::ErrorKind::InvalidClientConfig,
                "Invalid Redis URL format",
            ))));
        }

        // Create a Redis client
        let client = Client::open(connection_url).map_err(RateLimitError::RedisError)?;

        // Use tokio timeout to prevent hanging on connection attempt
        let connection_manager_result =
            timeout(CONNECTION_TIMEOUT, ConnectionManager::new(client)).await;

        // Handle timeout and connection errors
        let connection_manager = match connection_manager_result {
            Ok(result) => result.map_err(RateLimitError::RedisError)?,
            Err(_) => return Err(RateLimitError::ConnectionTimeout),
        };

        // Create the Lua script once when initializing the rate limiter
        let increment_script = Script::new(
            r#"
            local current = redis.call('INCR', KEYS[1])
            if redis.call('TTL', KEYS[1]) < 0 then
                redis.call('EXPIRE', KEYS[1], ARGV[1])
            end
            return current
            "#,
        );

        Ok(Self {
            connection: Arc::new(connection_manager),
            increment_script,
        })
    }

    /// Bans an IP address for the specified duration
    pub async fn ban_ip(
        &self,
        ip: &str,
        reason: &str,
        duration: Duration,
    ) -> Result<(), RateLimitError> {
        let ban_key = format!("ban:ip:{}", ip);
        let timeout_secs = duration.as_secs();
        let mut connection = (*self.connection).clone();

        // Set the ban with the reason as the value
        let _: () = redis::cmd("SET")
            .arg(&ban_key)
            .arg(reason)
            .arg("EX")
            .arg(timeout_secs)
            .query_async(&mut connection)
            .await
            .map_err(RateLimitError::RedisError)?;

        Ok(())
    }

    /// Checks if an IP address is banned
    pub async fn is_banned(&self, ip: &str) -> Result<Option<String>, RateLimitError> {
        let ban_key = format!("ban:ip:{}", ip);
        let mut connection = (*self.connection).clone();

        // Get the ban reason if it exists
        let result: Option<String> = redis::cmd("GET")
            .arg(&ban_key)
            .query_async(&mut connection)
            .await
            .map_err(RateLimitError::RedisError)?;

        Ok(result)
    }
}

#[async_trait]
impl RateLimiter for RedisRateLimiter {
    async fn increment(&self, key: &str, timeout: Duration) -> Result<u64, RateLimitError> {
        let timeout_secs = timeout.as_secs();
        let mut connection = (*self.connection).clone();

        // Use the pre-compiled script (atomic approach)
        match self
            .increment_script
            .key(key)
            .arg(timeout_secs)
            .invoke_async(&mut connection)
            .await
        {
            Ok(value) => Ok(value),
            Err(err) => {
                // Log the error but return 0 to fail open
                tracing::warn!("Redis rate limit error: {}", err);
                Ok(0)
            }
        }
    }

    async fn check_limit(
        &self,
        key: &str,
        limit: u64,
        timeout: Duration,
    ) -> Result<u64, RateLimitError> {
        match self.increment(key, timeout).await {
            Ok(count) if count <= limit => Ok(count),
            Ok(count) if count > limit => Err(RateLimitError::RateLimitExceeded { limit, count }),
            // For any error or other case, fail open
            _ => Ok(0),
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// An entry in the in-memory rate limiter
#[derive(Debug)]
struct RateLimitEntry {
    count: u64,
    expires_at: Instant,
}

/// An in-memory implementation of the RateLimiter trait
#[derive(Debug)]
pub struct InMemoryRateLimiter {
    entries: RwLock<HashMap<String, RateLimitEntry>>,
}

impl InMemoryRateLimiter {
    /// Creates a new in-memory rate limiter
    pub fn new() -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
        }
    }

    /// Cleans up expired entries
    async fn cleanup(&self) {
        // Try to get the write lock, but fail open if we can't
        if let Ok(mut entries) = self.entries.try_write() {
            let now = Instant::now();
            entries.retain(|_, entry| entry.expires_at > now);
        }
    }

    /// Bans an IP address for the specified duration
    pub async fn ban_ip(
        &self,
        ip: &str,
        _: &str,
        duration: Duration,
    ) -> Result<(), RateLimitError> {
        let now = Instant::now();
        let ban_key = format!("ban:ip:{}", ip);

        let mut entries = self.entries.try_write().map_err(|e| {
            tracing::error!("Failed to acquire write lock: {}", e);
            RateLimitError::LockError
        })?;

        entries.insert(
            ban_key,
            RateLimitEntry {
                count: 1, // Use count=1 to indicate banned
                expires_at: now + duration,
            },
        );

        Ok(())
    }

    /// Checks if an IP address is banned
    pub async fn is_banned(&self, ip: &str) -> Result<Option<String>, RateLimitError> {
        let now = Instant::now();
        let ban_key = format!("ban:ip:{}", ip);

        let entries = self.entries.try_read().map_err(|e| {
            tracing::error!("Failed to acquire read lock: {}", e);
            RateLimitError::LockError
        })?;

        if let Some(entry) = entries.get(&ban_key) {
            if entry.expires_at > now {
                // IP is banned, return a generic reason since we don't store reasons
                return Ok(Some("IP address banned".to_string()));
            }
        }

        Ok(None)
    }
}

impl Default for InMemoryRateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl RateLimiter for InMemoryRateLimiter {
    async fn increment(&self, key: &str, timeout: Duration) -> Result<u64, RateLimitError> {
        // Periodically clean up expired entries
        if rand::rng().random_bool(0.01) {
            // 1% chance on each call
            self.cleanup().await;
        }

        let now = Instant::now();

        let mut entries = self.entries.write().await;

        let entry = entries
            .entry(key.to_string())
            .or_insert_with(|| RateLimitEntry {
                count: 0,
                expires_at: now + timeout,
            });

        // Update expiry time if it's an existing entry
        entry.expires_at = now + timeout;
        entry.count += 1;

        Ok(entry.count)
    }

    async fn check_limit(
        &self,
        key: &str,
        limit: u64,
        timeout: Duration,
    ) -> Result<u64, RateLimitError> {
        match self.increment(key, timeout).await {
            Ok(count) if count <= limit => Ok(count),
            Ok(count) if count > limit => Err(RateLimitError::RateLimitExceeded { limit, count }),
            // For any error or other case, fail open
            _ => Ok(0),
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Enum to represent different types of rate limiters that can ban IPs
#[derive(Debug, Clone)]
pub enum BanManager {
    Redis(Arc<RedisRateLimiter>),
    InMemory(Arc<InMemoryRateLimiter>),
}

impl BanManager {
    /// Bans an IP address for the specified duration
    pub async fn ban_ip(
        &self,
        ip: &str,
        reason: &str,
        duration: Duration,
    ) -> Result<(), RateLimitError> {
        match self {
            BanManager::Redis(limiter) => limiter.ban_ip(ip, reason, duration).await,
            BanManager::InMemory(limiter) => limiter.ban_ip(ip, reason, duration).await,
        }
    }

    /// Checks if an IP address is banned
    pub async fn is_banned(&self, ip: &str) -> Result<Option<String>, RateLimitError> {
        match self {
            BanManager::Redis(limiter) => limiter.is_banned(ip).await,
            BanManager::InMemory(limiter) => limiter.is_banned(ip).await,
        }
    }
}

/// Utility function to create a rate limit key for a specific minute
pub fn create_rate_limit_key(api_key: &str, resource: &str) -> String {
    // Get the current minute number (0-59)
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();

    let minutes = now.as_secs() / 60 % 60;
    format!("ratelimit:{}:{}:{}", api_key, resource, minutes)
}

/// Utility function to create a ban key for an IP address
pub fn create_ban_key(ip: &str) -> String {
    format!("ban:ip:{}", ip)
}

// Global map of URL to mutex to ensure only one connection attempt per URL at a time
static CONNECTION_LOCKS: LazyLock<Mutex<HashMap<String, Arc<AsyncMutex<()>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// A global store for rate limiters, indexed by connection URL
pub struct RateLimiterStore {
    redis_limiters: Mutex<HashMap<String, Arc<RedisRateLimiter>>>,
    memory_limiter: Arc<InMemoryRateLimiter>,
    // Track known bad Redis URLs to avoid repeated connection attempts
    failed_urls: Mutex<HashSet<String>>,
}

impl RateLimiterStore {
    /// Create a new store with a single in-memory rate limiter
    fn new() -> Self {
        Self {
            redis_limiters: Mutex::new(HashMap::new()),
            memory_limiter: Arc::new(InMemoryRateLimiter::new()),
            failed_urls: Mutex::new(HashSet::new()),
        }
    }

    /// Get an in-memory rate limiter
    pub fn get_memory_limiter(&self) -> Arc<InMemoryRateLimiter> {
        self.memory_limiter.clone()
    }

    /// Get a Redis rate limiter for the given connection URL, creating one if it doesn't exist
    pub async fn get_redis_limiter(
        &self,
        connection_url: &str,
    ) -> Result<Arc<RedisRateLimiter>, RateLimitError> {
        // First check if this URL is known to fail
        {
            let failed_urls = self.failed_urls.lock().unwrap_or_else(|e| e.into_inner());
            if failed_urls.contains(connection_url) {
                return Err(RateLimitError::ConnectionTimeout);
            }
        }

        // Then check if we already have a limiter for this URL
        {
            let limiters = self
                .redis_limiters
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if let Some(limiter) = limiters.get(connection_url) {
                return Ok(limiter.clone());
            }
        }

        // Get a dedicated mutex for this URL or create a new one if it doesn't exist
        let url_mutex = {
            let mut locks = CONNECTION_LOCKS.lock().unwrap_or_else(|e| e.into_inner());

            // Get or create the mutex for this URL
            locks
                .entry(connection_url.to_string())
                .or_insert_with(|| Arc::new(AsyncMutex::new(())))
                .clone()
        };

        // Acquire the mutex with a timeout to avoid deadlocks
        let lock_result = timeout(Duration::from_secs(5), url_mutex.lock()).await;
        let _guard = match lock_result {
            Ok(guard) => guard,
            Err(_) => {
                tracing::warn!("Timed out waiting for lock on URL: {}", connection_url);
                return Err(RateLimitError::LockError);
            }
        };

        // Check again if another thread created the limiter while we were waiting
        {
            let limiters = self
                .redis_limiters
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if let Some(limiter) = limiters.get(connection_url) {
                return Ok(limiter.clone());
            }
        }

        // Create a new limiter
        tracing::info!("Initializing Redis rate limiter for {}", connection_url);
        match RedisRateLimiter::new(connection_url).await {
            Ok(limiter) => {
                let limiter = Arc::new(limiter);

                // Store it for future use
                let mut limiters = self
                    .redis_limiters
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                limiters.insert(connection_url.to_string(), limiter.clone());

                Ok(limiter)
            }
            Err(e) => {
                tracing::error!("Failed to initialize Redis rate limiter: {}", e);
                // Cache the failure
                let mut failed_urls = self.failed_urls.lock().unwrap_or_else(|e| e.into_inner());
                failed_urls.insert(connection_url.to_string());
                Err(e)
            }
        }
    }

    /// Get a BanManager for the given RateLimiterConfig
    pub async fn get_ban_manager(
        &self,
        config: &RateLimiterConfig,
    ) -> Result<BanManager, RateLimitError> {
        match config {
            RateLimiterConfig::Memory => {
                tracing::debug!("Using in-memory ban manager");
                Ok(BanManager::InMemory(self.get_memory_limiter()))
            }
            RateLimiterConfig::Redis { connection_url } => {
                match self.get_redis_limiter(connection_url).await {
                    Ok(limiter) => Ok(BanManager::Redis(limiter)),
                    Err(_) => Ok(BanManager::InMemory(self.get_memory_limiter())),
                }
            }
        }
    }
}

/// Global store of rate limiters
pub static RATE_LIMITER_STORE: LazyLock<RateLimiterStore> = LazyLock::new(RateLimiterStore::new);

/// Convenience function to get an in-memory rate limiter
pub fn get_memory_rate_limiter() -> Arc<impl RateLimiter> {
    RATE_LIMITER_STORE.get_memory_limiter()
}

/// Convenience function to get a Redis rate limiter by connection URL
pub async fn get_redis_rate_limiter(
    connection_url: &str,
) -> Result<Arc<impl RateLimiter>, RateLimitError> {
    RATE_LIMITER_STORE.get_redis_limiter(connection_url).await
}

/// Get a rate limiter based on configuration
pub async fn get_rate_limiter(
    config: &RateLimiterConfig,
) -> Result<Arc<dyn RateLimiter>, RateLimitError> {
    match config {
        RateLimiterConfig::Memory => Ok(get_memory_rate_limiter() as Arc<dyn RateLimiter>),
        RateLimiterConfig::Redis { connection_url } => {
            match get_redis_rate_limiter(connection_url).await {
                Ok(limiter) => Ok(limiter as Arc<dyn RateLimiter>),
                Err(_) => Ok(get_memory_rate_limiter() as Arc<dyn RateLimiter>),
            }
        }
    }
}

/// Get a ban manager based on configuration
pub async fn get_ban_manager(config: &RateLimiterConfig) -> Result<BanManager, RateLimitError> {
    RATE_LIMITER_STORE.get_ban_manager(config).await
}

/// Configuration for rate limiters
#[derive(Debug, Clone, Deserialize)]
pub enum RateLimiterConfig {
    /// Use an in-memory rate limiter
    #[serde(rename(deserialize = "in_memory"))]
    Memory,
    /// Use a Redis-backed rate limiter
    #[serde(rename(deserialize = "redis"))]
    Redis {
        /// Connection URL, including database number if needed (e.g., "redis://localhost:6379/0")
        connection_url: String,
    },
}
