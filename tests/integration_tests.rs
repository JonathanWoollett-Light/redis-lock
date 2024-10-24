use redis::AsyncCommands as _;
use redis::Client;
use serial_test::serial;
use std::error::Error;
use std::process::Stdio;

#[cfg(feature = "sync")]
use redis::Commands as _;

#[cfg(feature = "sync")]
const ONE: &str = env!("CARGO_BIN_EXE_one");
#[cfg(feature = "sync")]
const TWO: &str = env!("CARGO_BIN_EXE_two");
const THREE: &str = env!("CARGO_BIN_EXE_three");
const FOUR: &str = env!("CARGO_BIN_EXE_four");
const FIVE: &str = env!("CARGO_BIN_EXE_five");

#[expect(
    clippy::panic_in_result_fn,
    reason = "It's annoying to handle the error here."
)]
#[expect(
    clippy::tests_outside_test_module,
    reason = "`#[serial]` break this lint"
)] // TODO Fix this.
#[test]
#[serial]
fn two() -> Result<(), Box<dyn Error>> {
    tokio::runtime::Runtime::new()?.block_on(async {
        const N: usize = 10;
        let redis_url = "redis://127.0.0.1/";
        let client = Client::open(redis_url)?;
        let mut conn = client.get_multiplexed_async_connection().await?;
        // Initialize account balances.
        redis::cmd("FLUSHALL").exec_async(&mut conn).await?;
        conn.set::<_, _, ()>("account1", 1000i32).await?;
        conn.set::<_, _, ()>("account2", 1000i32).await?;
        conn.set::<_, _, ()>("account3", 1000i32).await?;
        // Loads functions.
        redis_lock::setup(&client).await?;
        // Executes multiple instances of `one.rs` and `two.rs`.
        let threes = (0..N)
            .map(|_| {
                tokio::process::Command::new(THREE)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
            })
            .collect::<Result<Vec<_>, _>>()?;
        let fours = (0..N)
            .map(|_| {
                tokio::process::Command::new(FOUR)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
            })
            .collect::<Result<Vec<_>, _>>()?;
        let fives = (0..N)
            .map(|_| {
                tokio::process::Command::new(FIVE)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
            })
            .collect::<Result<Vec<_>, _>>()?;

        // Waits for all instances to finish.
        for three in threes {
            let output = three.wait_with_output().await?;
            assert!(output.status.success());
            assert!(output.stderr.is_empty());
        }
        for four in fours {
            let output = four.wait_with_output().await?;
            assert!(output.status.success());
            assert!(output.stderr.is_empty());
        }
        for five in fives {
            let output = five.wait_with_output().await?;
            assert!(output.status.success());
            assert!(output.stderr.is_empty());
        }

        let balance1: i64 = conn.get("account1").await?;
        let balance2: i64 = conn.get("account2").await?;
        let balance3: i64 = conn.get("account3").await?;
        let total_balance = balance1
            .checked_add(balance2)
            .ok_or("overflow")?
            .checked_add(balance3)
            .ok_or("overflow")?;
        if total_balance == 3000 {
            Ok(())
        } else {
            Err("Total balance is not 3000".into())
        }
    })
}

// https://www.perplexity.ai/search/is-it-possibly-to-implement-di-PXg_TYNAQ5GfBStsVB1qAw
#[expect(clippy::tests_outside_test_module, reason = "`#[serial]` breaks it")] // TODO Fix this.
#[cfg(feature = "sync")]
#[test]
#[serial]
fn one() -> Result<(), Box<dyn Error>> {
    const N: usize = 10;

    let redis_url = "redis://127.0.0.1/";
    let client = Client::open(redis_url)?;
    let mut conn = client.get_connection()?;
    // Initialize account balances.
    redis::cmd("FLUSHALL").exec(&mut conn)?;
    conn.set::<_, _, ()>("account1", 1000i32)?;
    conn.set::<_, _, ()>("account2", 1000i32)?;
    conn.set::<_, _, ()>("account3", 1000i32)?;
    // Loads functions.
    redis_lock::sync::setup(&client)?;
    // Executes multiple instances of `one.rs` and `two.rs`.
    let ones = (0..N)
        .map(|_| {
            std::process::Command::new(ONE)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
        })
        .collect::<Result<Vec<_>, _>>()?;
    let twos = (0..N)
        .map(|_| {
            std::process::Command::new(TWO)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
        })
        .collect::<Result<Vec<_>, _>>()?;
    // Waits for all instances to finish.
    for one in ones {
        let _output = one.wait_with_output()?;
    }
    for two in twos {
        let _output = two.wait_with_output()?;
    }

    let balance1: i64 = conn.get("account1")?;
    let balance2: i64 = conn.get("account2")?;
    let balance3: i64 = conn.get("account3")?;
    let total_balance = balance1
        .checked_add(balance2)
        .ok_or("overflow")?
        .checked_add(balance3)
        .ok_or("overflow")?;
    if total_balance == 3000 {
        Ok(())
    } else {
        Err("Total balance is not 3000".into())
    }
}
