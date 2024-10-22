//! Rusty distributed locking backed by Redis.
//!
//! ## Similar work
//!
//! - <https://github.com/hexcowboy/rslock>
use redis::{Client, Connection, RedisResult};
use std::error::Error;
use std::time::Duration;
use uuid::Uuid;

/// A distributed mutual exclusion lock backed by Redis.
///
/// Supports exclusion based on multiple resources and partial overlaps.
///
/// E.g. a lock on resources `["a", "b"]` will block a lock on `["a"]` or `["b", "c"]`.
pub struct MultiResourceLock {
    /// The Redis connection.
    conn: Connection,
}

impl std::fmt::Debug for MultiResourceLock {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MultiResourceLock")
            .field("conn", &"..")
            .finish()
    }
}

/// Initializes a Redis instance with the Lua library functions required for locking.
///
/// # Errors
///
/// - When [`Client::get_connection`] errors.
/// - When the Lua library functions cannot be loaded into Redis.
#[inline]
pub fn setup(client: &Client) -> Result<(), Box<dyn Error>> {
    // Connect to Redis
    let mut con = client.get_connection()?;

    // Define your Lua library
    let lua_library = include_str!("functions.lua");

    // Load the Lua library into Redis
    let _result: String = redis::cmd("FUNCTION")
        .arg("LOAD")
        .arg("REPLACE")
        .arg(lua_library)
        .query(&mut con)?;

    Ok(())
}

impl MultiResourceLock {
    /// Create a new instance of the lock.
    ///
    /// # Errors
    ///
    /// When [`Client::get_connection`] errors.
    #[inline]
    pub fn new(client: &Client) -> RedisResult<Self> {
        let conn = client.get_connection()?;
        Ok(MultiResourceLock { conn })
    }

    /// Attempts to acquire the lock blocking up to `expiration` until the lock can be acquired.
    ///
    /// Returns `None` when it times out.
    ///
    /// # Errors
    ///
    /// When [`MultiResourceLock::try_acquire`] errors.
    #[inline]
    pub fn acquire(
        &mut self,
        resources: &[String],
        expiration: Duration,
        timeout: Duration,
        sleep: Duration,
    ) -> RedisResult<Option<String>> {
        let now = std::time::Instant::now();
        loop {
            if now.elapsed() > timeout {
                return Ok(None);
            }
            match self.try_acquire(resources, expiration)? {
                Some(res) => break Ok(Some(res)),
                None => std::thread::sleep(sleep),
            }
        }
    }

    /// Attempts to acquire the lock returning immediately if it cannot be immediately acquired.
    ///
    /// # Errors
    ///
    /// - When the `acquire_lock` function is missing from the Redis instance.
    #[inline]
    pub fn try_acquire(
        &mut self,
        resources: &[String],
        expiration: Duration,
    ) -> RedisResult<Option<String>> {
        let lock_id = Uuid::new_v4().to_string();
        let mut args = vec![lock_id.clone(), expiration.as_millis().to_string()];
        args.extend(resources.iter().cloned());

        let result: Option<String> = redis::cmd("FCALL")
            .arg("acquire_lock")
            .arg(&args)
            .query(&mut self.conn)?;

        Ok(result)
    }

    /// Releases a held lock.
    ///
    /// # Errors
    ///
    /// - When the `release_lock` function is missing from the Redis instance.
    /// - When `lock_id` is does not refer to a held lock.
    #[inline]
    pub fn release(&mut self, lock_id: &str) -> RedisResult<usize> {
        let result: usize = redis::cmd("FCALL")
            .arg("release_lock")
            .arg(lock_id)
            .query(&mut self.conn)?;

        Ok(result)
    }

    /// Attempts to acquire the lock returning immediately if it cannot be immediately acquired.
    ///
    /// # Errors
    ///
    /// When [`MultiResourceLock::try_acquire`] errors.
    #[inline]
    pub fn try_lock(
        &mut self,
        resources: &[String],
        expiration: Duration,
    ) -> RedisResult<Option<MultiResourceGuard<'_>>> {
        self.try_acquire(resources, expiration).map(|result| {
            result.map(|lock_id| MultiResourceGuard {
                lock: self,
                lock_id,
            })
        })
    }

    /// Attempts to acquire the lock blocking up to `expiration` until the lock can be acquired.
    ///
    /// Returns `None` when it times out.
    ///
    /// # Errors
    ///
    /// When [`MultiResourceLock::acquire`] errors.
    #[inline]
    pub fn lock(
        &mut self,
        resources: &[String],
        expiration: Duration,
        timeout: Duration,
        sleep: Duration,
    ) -> RedisResult<Option<MultiResourceGuard<'_>>> {
        self.acquire(resources, expiration, timeout, sleep)
            .map(|result| {
                result.map(|lock_id| MultiResourceGuard {
                    lock: self,
                    lock_id,
                })
            })
    }
}

/// A guard that releases the lock when it is dropped.
#[derive(Debug)]
pub struct MultiResourceGuard<'a> {
    /// The lock instance.
    lock: &'a mut MultiResourceLock,
    /// The lock identifier.
    lock_id: String,
}

#[expect(
    clippy::unwrap_used,
    reason = "You can't propagate errors in a `Drop` implementation."
)]
impl Drop for MultiResourceGuard<'_> {
    #[inline]
    fn drop(&mut self) {
        self.lock.release(&self.lock_id).unwrap();
    }
}
