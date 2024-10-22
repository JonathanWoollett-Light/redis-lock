//! Test binary.

use rand::Rng;
use redis::{Client, Commands, Connection};
use redis_lock::MultiResourceLock;
use std::error::Error;
use std::time::Duration;

/// Executes a transfer from one account to another.
#[expect(
    dependency_on_unit_never_type_fallback,
    reason = "I'm pretty sure this can only be fixed with a change to the `redis` crate."
)]
fn transfer(
    conn: &mut Connection,
    from: &str,
    to: &str,
    amount: i64,
) -> Result<(), Box<dyn Error>> {
    let from_balance: i64 = conn.get(from)?;

    // Only execute the transaction if the sender has enough funds.
    if from_balance >= amount {
        let to_balance: i64 = conn.get(to)?;
        conn.set(from, from_balance.checked_sub(amount).ok_or("underflow")?)?;
        conn.set(to, to_balance.checked_add(amount).ok_or("overflow")?)?;
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let redis_url = "redis://127.0.0.1/";
    let client = Client::open(redis_url)?;
    let mut lock = MultiResourceLock::new(&client)?;
    let mut conn = client.get_connection()?;

    let mut rng = rand::thread_rng();

    for _ in 0..10usize {
        let amount = rng.gen_range(10..=100);
        let resources = vec![String::from("account1"), String::from("account2")];

        // Try to acquire the lock
        let opt = lock.acquire(
            &resources,
            Duration::from_secs(60),
            Duration::from_secs(60),
            Duration::from_secs(1),
        )?;
        let lock_id = opt.ok_or("timed out")?;
        // Lock acquired, perform the transfer
        transfer(&mut conn, "account1", "account2", amount)?;

        // Release the lock
        lock.release(&lock_id)?;
    }

    Ok(())
}
