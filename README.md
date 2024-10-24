# redis-lock

[![Crates.io](https://img.shields.io/crates/v/redis-lock)](https://crates.io/crates/redis-lock)
[![docs](https://img.shields.io/crates/v/redis-lock?color=yellow&label=docs)](https://docs.rs/redis-lock)

Rusty distributed locking backed by Redis.

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

I would recommend this library over [rslock](https://github.com/hexcowboy/rslock) when your
application does operations that require exclusive access to multiple resources.

## Similar work

- https://github.com/hexcowboy/rslock