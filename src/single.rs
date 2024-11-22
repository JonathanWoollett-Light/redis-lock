use std::future::Future;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tokio::sync::MutexGuard;
use tracing::{trace, warn};

/// The prefix for the lock key.
const LOCK_KEY_PREFIX: &str = "redlock:";
/// The default time-to-live for the lock.
pub const DEFAULT_TTL: Duration = Duration::from_millis(10);
/// The default delay between retries when attempting to acquire the lock.
pub const DEFAULT_RETRY_DELAY: Duration = Duration::from_millis(100);
/// The default duration to attempt to acquire the lock.
pub const DEFAULT_DURATION: Duration = Duration::from_secs(10);
/// The clock drift factor.
const CLOCK_DRIFT_FACTOR: f64 = 0.01;

/// Lock metadata.
// TODO Remove this allow.
#[expect(dead_code, reason = "see todo")]
struct Lock {
    /// The name of the resource to lock.
    resource: String,
    /// The unique value of the lock.
    value: String,
    /// The time the lock is valid for.
    validity_time: Duration,
}

/// Options to configure [`lock_across`].
#[derive(Debug)]
pub struct LockAcrossOptions {
    /// The time-to-live for the lock.
    pub ttl: Duration,
    /// The delay between retries when attempting to acquire the lock.
    pub retry: Duration,
    /// The maximum duration to attempt to acquire the lock.
    pub duration: Duration,
}
impl Default for LockAcrossOptions {
    #[inline]
    fn default() -> Self {
        Self {
            ttl: DEFAULT_TTL,
            retry: DEFAULT_RETRY_DELAY,
            duration: DEFAULT_DURATION,
        }
    }
}

/// Executes a function while locking on a single resource using the
/// [RedLock algorithm](https://redis.io/docs/latest/develop/use/patterns/distributed-locks/).
///
/// This is much more efficient than [`crate::MultiResourceLock`] when you only need to lock a single
/// resource. Ideally you should architect your application so you never need [`crate::MultiResourceLock`].
///
/// - `connections` is used to acquire mutable references on connections to acquire the lock and
///   then used to acquire mutable references on connections to release the lock.
/// - `resource` is the name of the resource to lock.
/// - `options` the options to configure acquisition.
///
/// ```no_run
/// # use tokio::{task, sync::Mutex};
/// # use std::sync::Arc;
/// # use redis_lock::{lock_across, LockAcrossOptions};
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # tokio::runtime::Runtime::new()?.block_on(async {
/// # let client: redis::Client = todo!();
/// // Get connection.
/// let connection = Arc::new(Mutex::new(client.get_multiplexed_async_connection().await?));
/// // Set state.
/// let x: usize = 0;
/// let ptr = &mut x as *mut usize as usize;
/// // Execute racy functions with lock.
/// const N: usize = 100_000;
/// let futures = (0..N).map(|_|{
///     let cconnection = connection.clone();
///     task::spawn(async move {
///         lock_across(
///             &[cconnection],
///             "resource",
///             async move {
///                 unsafe { *(ptr as *mut usize) += 1 };
///             },
///             LockAcrossOptions::default(),
///         );
///     })
/// }).collect::<Vec<_>>();
/// for future in futures {
///     future.await?;
/// }
/// // Assert state.
/// assert_eq!(x, N);
/// # Ok(())
/// # })
/// # }
/// ```
#[inline]
pub async fn lock_across<C, F>(
    connections: &[Arc<Mutex<C>>],
    resource: &str,
    f: F,
    options: LockAcrossOptions,
) -> Result<F::Output, redis::RedisError>
where
    C: redis::aio::ConnectionLike,
    F: Future + 'static,
    F::Output: 'static,
{
    trace!("acquiring lock");
    let lock_opt = acquire_lock(connections, resource, options).await?;

    if let Some(lock) = lock_opt {
        trace!("acquired lock");

        // Execute the provided function
        let output = f.await;
        trace!("executed function");

        // Get the connections back for releasing the lock
        let mut release_connections = Vec::new();
        for conn_future in connections {
            trace!("getting connection");
            release_connections.push(conn_future.lock().await);
            trace!("acquired connection");
        }

        // Release the lock
        trace!("releasing lock");
        release_lock(&mut release_connections, &lock).await?;
        trace!("released lock");
        Ok(output)
    } else {
        Err(redis::RedisError::from((
            redis::ErrorKind::IoError,
            "Failed to acquire lock",
        )))
    }
}

/// Attempts to acquire a lock on multiple connections.
#[expect(
    clippy::as_conversions,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::float_arithmetic,
    clippy::cast_sign_loss,
    clippy::arithmetic_side_effects,
    clippy::integer_division_remainder_used,
    clippy::integer_division,
    reason = "I can't be bothered to fix these right now."
)]
async fn acquire_lock<'a, C: redis::aio::ConnectionLike>(
    connections: &[Arc<Mutex<C>>],
    resource: &str,
    options: LockAcrossOptions,
) -> Result<Option<Lock>, redis::RedisError> {
    let value = uuid::Uuid::new_v4().to_string();
    let resource_key = format!("{LOCK_KEY_PREFIX}{resource}");
    let quorum = (connections.len() / 2) + 1;
    let outer_start = Instant::now();
    let mut attempts = 0u64;
    let ttl_millis = options.ttl.as_millis() as u64;

    while outer_start.elapsed() < options.duration {
        attempts += 1;
        trace!("Attempting to acquire lock (attempt {attempts})");
        let mut successful_locks = Vec::new();
        let start = Instant::now();

        for conn_future in connections {
            trace!("getting connection");
            let mut conn = conn_future.lock().await;
            if let Ok(true) = try_acquire_lock(&mut *conn, &resource_key, &value, ttl_millis).await
            {
                trace!("acquired lock");
                successful_locks.push(conn);
            }
        }

        let drift = (ttl_millis as f64 * CLOCK_DRIFT_FACTOR + 2.0f64) as u64;
        let elapsed = start.elapsed().as_millis() as u64;
        let validity_time = ttl_millis.saturating_sub(elapsed).saturating_sub(drift);

        if successful_locks.len() >= quorum && validity_time > 0 {
            trace!("Lock acquired successfully");
            return Ok(Some(Lock {
                resource: resource_key,
                value,
                validity_time: Duration::from_millis(validity_time),
            }));
        }
        trace!(
            "Failed to acquire lock, releasing {} successful locks",
            successful_locks.len()
        );
        for mut conn in successful_locks {
            if let Err(e) = release_lock_on_connection(&mut *conn, &resource_key, &value).await {
                trace!("Error releasing lock: {:?}", e);
            }
        }

        trace!("Waiting before next attempt");
        tokio::time::sleep(options.retry).await;
    }

    warn!(
        "Failed to acquire lock after {:?} and {attempts} attempts",
        options.duration
    );
    Ok(None)
}

/// Attempts to acquire a lock on a connection.
async fn try_acquire_lock<C: redis::aio::ConnectionLike>(
    conn: &mut C,
    resource: &str,
    value: &str,
    ttl: u64,
) -> Result<bool, redis::RedisError> {
    let result: Option<String> = redis::cmd("SET")
        .arg(resource)
        .arg(value)
        .arg("NX")
        .arg("PX")
        .arg(ttl)
        .query_async(conn)
        .await?;

    Ok(result.is_some())
}

/// Releases a lock on multiple connections.
async fn release_lock<C: redis::aio::ConnectionLike>(
    connections: &mut [MutexGuard<'_, C>],
    lock: &Lock,
) -> Result<(), redis::RedisError> {
    for conn in connections.iter_mut() {
        let x: &mut C = &mut *conn;
        release_lock_on_connection(x, &lock.resource, &lock.value).await?;
    }
    Ok(())
}

/// Releases a lock on a connection.
async fn release_lock_on_connection<C: redis::aio::ConnectionLike>(
    conn: &mut C,
    resource: &str,
    value: &str,
) -> Result<(), redis::RedisError> {
    let script = r#"
        if redis.call("get", KEYS[1]) == ARGV[1] then
            return redis.call("del", KEYS[1])
        else
            return 0
        end
    "#;

    let _: () = redis::cmd("EVAL")
        .arg(script)
        .arg(1i32)
        .arg(resource)
        .arg(value)
        .query_async(conn)
        .await?;

    Ok(())
}
