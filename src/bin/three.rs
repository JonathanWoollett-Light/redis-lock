use rand::Rng;
use redis::{Client, Commands, Connection, RedisResult};
use redis_locking::MultiResourceLock;
use std::error::Error;
use std::thread;
use std::time::Duration;

fn transfer(conn: &mut Connection, from: &str, to: &str, amount: i64) -> RedisResult<()> {
    let from_balance: i64 = conn.get(from)?;

    // Simulate some processing time
    thread::sleep(Duration::from_millis(100));

    if from_balance >= amount {
        let to_balance: i64 = conn.get(to)?;
        conn.set(from, from_balance - amount)?;
        conn.set(to, to_balance + amount)?;
        println!("Transferred {} from {} to {}", amount, from, to);
    } else {
        println!("Insufficient funds in {}", from);
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let redis_url = "redis://127.0.0.1/";
    let client = Client::open(redis_url)?;
    let mut lock = MultiResourceLock::new(&client)?;
    let mut conn = client.get_connection()?;

    let mut rng = rand::thread_rng();

    for _ in 0..15 {
        let amount = rng.gen_range(10..=100);
        let transfer_type = rng.gen_range(0..3);

        let (from, to, resources) = match transfer_type {
            0 => (
                "account1",
                "account2",
                vec!["account1".to_string(), "account2".to_string()],
            ),
            1 => (
                "account2",
                "account3",
                vec!["account2".to_string(), "account3".to_string()],
            ),
            _ => (
                "account1",
                "account3",
                vec!["account1".to_string(), "account3".to_string()],
            ),
        };

        // Try to acquire the lock
        if let Some(_lock_id) = lock.lock(&resources, 10)? {
            // Lock acquired, perform the transfer
            transfer(&mut conn, from, to, amount)?;
        } else {
            println!("Failed to acquire lock for {} -> {}, retrying...", from, to);
            thread::sleep(Duration::from_millis(100));
        }

        // Small delay between operations
        thread::sleep(Duration::from_millis(50));
    }

    // Print final balances
    let balance1: i64 = conn.get("account1")?;
    let balance2: i64 = conn.get("account2")?;
    let balance3: i64 = conn.get("account3")?;
    println!(
        "Final balances: account1 = {}, account2 = {}, account3 = {}",
        balance1, balance2, balance3
    );
    println!("Total balance: {}", balance1 + balance2 + balance3);

    Ok(())
}
