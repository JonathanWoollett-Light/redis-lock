# redis-lock

[![Crates.io](https://img.shields.io/crates/v/redis-lock)](https://crates.io/crates/redis-lock)
[![docs](https://img.shields.io/crates/v/redis-lock?color=yellow&label=docs)](https://docs.rs/redis-lock)

Rusty distributed locking backed by Redis.

```rust
// Setup.
redis_lock::setup(&client).await?;
let mut lock = redis_lock::MultiResourceLock::new(client.clone())?;
let mut conn = client.get_multiplexed_async_connection().await?;
let from = "account1";
let to = "account2";
let resources = vec![String::from(from), String::from(to)];
// Acquire lock.
let opt = lock.lock_default(&resources).await?;
let guard = opt.ok_or("timed out")?;
// Perform transfer.
let amount = 500;
let from_balance: i64 = conn.get("account1").await?;
// Execute transaction if the sender has enough funds.
if from_balance >= amount {
    let to_balance: i64 = conn.get(to).await?;
    let new_from = from_balance.checked_sub(amount).ok_or("underflow")?;
    conn.set(from, new_from).await?;
    let new_to = to_balance.checked_add(amount).ok_or("overflow")?;
    conn.set(to, new_to).await?;
}
// Lock releases when dropped.
```

## Vs [rslock](https://github.com/hexcowboy/rslock)

I would recommend this library over [rslock](https://github.com/hexcowboy/rslock) when:
- your application is focussed on `async`.
- your application does operations that require exclusive access to multiple resources.

## Similar work

- https://github.com/hexcowboy/rslock