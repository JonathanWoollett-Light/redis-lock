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

/// Errors that can occur during a check.
#[derive(Debug, Display, Error)]
enum CheckError {
    /// Failed to open connection: {0}
    Connection(RedisError),
    /// Failed to get balance: {0}
    Get(RedisError),
}

/// Get the balance of an account.
async fn check(client: Client, from: &str) -> Result<i64, CheckError> {
    let mut conn = client
        .get_multiplexed_async_connection()
        .await
        .map_err(CheckError::Connection)?;
    let from_balance: i64 = conn.get(from).await.map_err(CheckError::Get)?;
    Ok(from_balance)
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

    for _ in 0..10usize {
        // Try to acquire the lock
        let cloned_client = client.clone();
        let _x = lock
            .map(
                &[String::from("account1")],
                Duration::from_secs(60),
                Duration::from_secs(2),
                Duration::from_millis(100),
                async move { check(cloned_client, "account1").await },
            )
            .await??;
    }

    Ok(())
}
