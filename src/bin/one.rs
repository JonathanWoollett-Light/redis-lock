//! Test binary.

use rand::Rng;
use redis::{Client, Commands, Connection, RedisResult};
use redis_lock::MultiResourceLock;
use std::error::Error;
use std::thread;
use std::time::Duration;

// I'm pretty sure this can only be fixed with a change to the `redis` crate.
#[allow(dependency_on_unit_never_type_fallback)]
fn transfer(conn: &mut Connection, from: &str, to: &str, amount: i64) -> RedisResult<()> {
    let from_balance: i64 = conn.get(from)?;

    // Simulate some processing time
    thread::sleep(Duration::from_millis(100));

    if from_balance >= amount {
        let to_balance: i64 = conn.get(to)?;
        conn.set(from, from_balance - amount)?;
        conn.set(to, to_balance + amount)?;
        println!("Transferred {amount} from {from} to {to}");
    } else {
        println!("Insufficient funds in {from}");
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

        println!("acquiring lock");

        // Try to acquire the lock
        let opt = lock.acquire(
            &resources,
            Duration::from_secs(60),
            Duration::from_secs(60),
            Duration::from_secs(1),
        )?;
        println!("opt: {opt:?}");
        let lock_id = opt.unwrap();
        // Lock acquired, perform the transfer
        transfer(&mut conn, "account1", "account2", amount)?;

        // Release the lock
        lock.release(&lock_id)?;

        // Small delay between operations
        thread::sleep(Duration::from_millis(50));
    }

    Ok(())
}
