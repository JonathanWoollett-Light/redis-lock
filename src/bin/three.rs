//! Test binary.

use rand::Rng;
use redis::aio::MultiplexedConnection;
use redis::{AsyncCommands, Client};
use redis_lock::MultiResourceLock;
use std::error::Error;
use std::time::Duration;

/// Executes a transfer from one account to another.
async fn transfer(
    conn: &mut MultiplexedConnection,
    from: &str,
    to: &str,
    amount: i64,
) -> Result<(), Box<dyn Error>> {
    let from_balance: i64 = conn.get(from).await?;

    // Only execute the transaction if the sender has enough funds.
    if from_balance >= amount {
        let to_balance: i64 = conn.get(to).await?;
        conn.set(from, from_balance.checked_sub(amount).ok_or("underflow")?)
            .await?;
        conn.set(to, to_balance.checked_add(amount).ok_or("overflow")?)
            .await?;
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let redis_url = "redis://127.0.0.1/";
    let client = Client::open(redis_url)?;
    let mut lock = MultiResourceLock::new(client.clone())?;
    let mut conn = client.get_multiplexed_async_connection().await?;
    let mut rng = rand::thread_rng();
    for _ in 0..10usize {
        let amount = rng.gen_range(10..=100);
        let resources = vec![String::from("account1"), String::from("account2")];

        // Try to acquire the lock
        let opt = lock
            .acquire(
                &resources,
                Duration::from_secs(60),
                Duration::from_secs(60),
                Duration::from_secs(1),
            )
            .await?;
        let lock_id = opt.ok_or("timed out")?;
        // Lock acquired, perform the transfer
        transfer(&mut conn, "account1", "account2", amount).await?;

        // Release the lock
        lock.release(&lock_id).await?;
    }

    Ok(())
}
