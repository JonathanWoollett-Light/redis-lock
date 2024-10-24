//! Test binary.

use displaydoc::Display;
use rand::distributions::Standard;
use rand::prelude::Distribution;
use rand::Rng;
use redis::AsyncCommands as _;
use redis::Client;
use redis::RedisError;
use redis_lock::MultiResourceLock;
use std::error::Error;
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Display, Error)]
enum TransferError {
    /// Failed to open connection: {0}
    Connection(RedisError),
    /// Failed to get from balance: {0}
    GetFrom(RedisError),
    /// Failed to get to balance: {0}
    GetTo(RedisError),
    /// Failed to set from balance: {0}
    SetFrom(RedisError),
    /// Failed to set to balance:{0}
    SetTo(RedisError),
}

/// Executes a transfer from one account to another.
#[expect(
    clippy::arithmetic_side_effects,
    reason = "We check the arithmetic is valid with the if statement."
)]
async fn transfer(client: Client, from: &str, to: &str, amount: i64) -> Result<(), TransferError> {
    let mut conn = client
        .get_multiplexed_async_connection()
        .await
        .map_err(TransferError::Connection)?;
    let from_balance: i64 = conn.get(from).await.map_err(TransferError::GetFrom)?;

    // Only execute the transaction if the sender has enough funds.
    if from_balance >= amount {
        let to_balance: i64 = conn.get(to).await.map_err(TransferError::GetTo)?;
        conn.set::<_, _, ()>(from, from_balance - amount)
            .await
            .map_err(TransferError::SetFrom)?;
        conn.set::<_, _, ()>(to, to_balance + amount)
            .await
            .map_err(TransferError::SetTo)?;
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let redis_url = "redis://127.0.0.1/";
    let client = Client::open(redis_url)?;
    let mut lock = MultiResourceLock::new(client.clone())?;

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
        let cloned_client = client.clone();
        lock.map(
            &resources,
            Duration::from_secs(60),
            Duration::from_secs(2),
            Duration::from_millis(100),
            async move { transfer(cloned_client, from, to, amount).await },
        )
        .await??;
    }

    Ok(())
}
