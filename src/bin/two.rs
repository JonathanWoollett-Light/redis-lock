use rand::distributions::Standard;
use rand::prelude::Distribution;
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
        println!("Transferred {} from {} to {}", amount, from, to);
    } else {
        println!("Insufficient funds in {}", from);
    }
    Ok(())
}

enum TransferType {
    Account1ToAccount2,
    Account2ToAccount3,
    Account1ToAccount3,
}

impl Distribution<TransferType> for Standard {
    #[inline]
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> TransferType {
        match rng.gen_range(0..3) {
            0 => TransferType::Account1ToAccount2,
            1 => TransferType::Account2ToAccount3,
            2 => TransferType::Account1ToAccount3,
            _ => unreachable!(),
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let redis_url = "redis://127.0.0.1/";
    let client = Client::open(redis_url)?;
    let mut lock = MultiResourceLock::new(&client)?;
    let mut conn = client.get_connection()?;

    let mut rng = rand::thread_rng();

    for _ in 0..10 {
        let amount = rng.gen_range(10..=100);
        let transfer_type = rng.gen();

        let (from, to, resources) = match transfer_type {
            TransferType::Account1ToAccount2 => (
                "account1",
                "account2",
                vec![String::from("account1"), String::from("account2")],
            ),
            TransferType::Account2ToAccount3 => (
                "account2",
                "account3",
                vec![String::from("account2"), String::from("account3")],
            ),
            TransferType::Account1ToAccount3 => (
                "account1",
                "account3",
                vec![String::from("account1"), String::from("account3")],
            ),
        };

        println!("acquiring lock");

        // Try to acquire the lock
        let opt = lock.lock(
            &resources,
            Duration::from_secs(60),
            Duration::from_secs(60),
            Duration::from_secs(1),
        )?;
        println!("opt: {opt:?}");
        let guard = opt.unwrap();

        // Lock acquired, perform the transfer
        transfer(&mut conn, from, to, amount)?;
        // Release the lock
        drop(guard);

        // Small delay between operations
        thread::sleep(Duration::from_millis(50));
    }

    Ok(())
}
