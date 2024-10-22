//! Test binary.

use rand::distributions::Standard;
use rand::prelude::Distribution;
use rand::Rng;
use redis::{Client, Commands, Connection};
use redis_lock::MultiResourceLock;
use std::error::Error;
use std::time::Duration;

/// Executes a transfer from one account to another.
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

/// The transfer operation to perform.
enum TransferType {
    /// Transfer from account 1 to account 2.
    Account1ToAccount2,
    /// Transfer from account 2 to account 3.
    Account2ToAccount3,
    /// Transfer from account 1 to account 3.
    Account1ToAccount3,
}

#[expect(clippy::unreachable, reason = "It's unreachable and it's a test")]
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

    for _ in 0..10usize {
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

        // Try to acquire the lock
        let opt = lock.lock(
            &resources,
            Duration::from_secs(60),
            Duration::from_secs(60),
            Duration::from_secs(1),
        )?;
        let guard = opt.ok_or("Timed out")?;

        // Lock acquired, perform the transfer
        transfer(&mut conn, from, to, amount)?;
        // Release the lock
        drop(guard);
    }

    Ok(())
}
