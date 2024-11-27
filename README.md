# redis-lock

[![Crates.io](https://img.shields.io/crates/v/redis-lock)](https://crates.io/crates/redis-lock)
[![docs](https://img.shields.io/crates/v/redis-lock?color=yellow&label=docs)](https://docs.rs/redis-lock)

Rusty distributed locking backed by Redis.

## Locking a single resource

```rust
let connection = Arc::new(Mutex::new(
    client.get_multiplexed_async_connection().await?
));
// Execute a function with the lock.
redis_lock::lock_across(
    &[connection],
    "account1",
    async move { /* .. */ },
    redis_lock::LockAcrossOptions::default()
).await?;
```

## Locking multiple resources

```rust
// Setup.
redis_lock::setup(&client).await?;
// Get lock.
let mut lock = redis_lock::MultiResourceLock::new(client.clone())?;
let resources = vec![String::from("account1"), String::from("account2")];
// Execute a function with the lock.
lock.map_default(&resources, async move { /* .. */ }).await?;
```

## Vs [rslock](https://github.com/hexcowboy/rslock)

I would recommend this library over [rslock](https://github.com/hexcowboy/rslock) when
- your application does operations that require exclusive access to multiple resources.
- you want to ensure the locks are freed without significantly impairing runtime performance[^1].

## Similar work

- https://github.com/hexcowboy/rslock

[^1]: `rslock::LockGuard` notes: <blockquote> Upon dropping the guard, LockManager::unlock will be ran synchronously on the executor.<br>This is known to block the tokio runtime if this happens inside of the context of a tokio runtime if tokio-comp is enabled as a feature on this crate or the redis crate. <br>To eliminate this risk, if the tokio-comp flag is enabled, the Drop impl will not be compiled, meaning that dropping the LockGuard will be a no-op. Under this circumstance, LockManager::unlock can be called manually using the inner lock at the appropriate point to release the lock taken in Redis. </blockquote>

