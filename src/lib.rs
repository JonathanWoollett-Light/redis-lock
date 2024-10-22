//! Rusty distributed locking backed by Redis.
//! 
//! ## Similar work
//! 
//! - <https://github.com/hexcowboy/rslock>
use redis::{Client, Connection, RedisResult};
use std::error::Error;
use std::time::Duration;
use uuid::Uuid;

pub struct MultiResourceLock {
    conn: Connection,
}

impl std::fmt::Debug for MultiResourceLock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MultiResourceLock")
            .field("conn", &"..")
            .finish()
    }
}

/// Initializes a Redis instance with the Lua library functions required for locking.
pub fn setup(client: &redis::Client) -> Result<(), Box<dyn Error>> {
    // Connect to Redis
    let mut con = client.get_connection()?;

    // Define your Lua library
    let lua_library = include_str!("functions.lua");

    // Load the Lua library into Redis
    let result: String = redis::cmd("FUNCTION")
        .arg("LOAD")
        .arg("REPLACE")
        .arg(lua_library)
        .query(&mut con)?;

    println!("Lua library loaded: {}", result);

    Ok(())
}

impl MultiResourceLock {
    /// Create a new instance of the lock.
    pub fn new(client: &Client) -> RedisResult<Self> {
        let conn = client.get_connection()?;
        Ok(MultiResourceLock { conn })
    }

    /// Attempts to acquire the lock blocking up to `expiration` until the lock can be acquired.
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
    pub fn release(&mut self, lock_id: &str) -> RedisResult<usize> {
        let result: usize = redis::cmd("FCALL")
            .arg("release_lock")
            .arg(lock_id)
            .query(&mut self.conn)?;

        Ok(result)
    }

    /// Attempts to acquire the lock returning immediately if it cannot be immediately acquired.
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
    lock: &'a mut MultiResourceLock,
    lock_id: String,
}

impl Drop for MultiResourceGuard<'_> {
    fn drop(&mut self) {
        self.lock.release(&self.lock_id).unwrap();
    }
}
