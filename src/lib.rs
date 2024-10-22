use redis::{Client, Commands, Connection, RedisResult};
use std::error::Error;
use uuid::Uuid;

pub struct MultiResourceLock {
    conn: Connection,
}

pub fn setup(client: &redis::Client) -> Result<(), Box<dyn Error>> {
    // Connect to Redis
    let mut con = client.get_connection()?;

    // Define your Lua library
    let lua_library = include_str!("functions.lua");

    // Load the Lua library into Redis
    let result: String = redis::cmd("FUNCTION")
        .arg("LOAD")
        .arg("REPLACE")
        .arg(lua_library)
        .query(&mut con)?;

    println!("Lua library loaded: {}", result);

    Ok(())
}

impl MultiResourceLock {
    pub fn new(client: &Client) -> RedisResult<Self> {
        let conn = client.get_connection()?;
        Ok(MultiResourceLock { conn })
    }

    pub fn acquire(
        &mut self,
        resources: &[String],
        expiration: usize,
    ) -> RedisResult<Option<String>> {
        let lock_id = Uuid::new_v4().to_string();
        let mut args = vec![lock_id.clone(), expiration.to_string()];
        args.extend(resources.iter().cloned());

        let result: Option<String> = redis::cmd("FCALL")
            .arg("acquire_lock")
            .arg(0)
            .arg(&args)
            .query(&mut self.conn)?;

        Ok(result)
    }

    pub fn release(&mut self, lock_id: &str) -> RedisResult<usize> {
        let result: usize = redis::cmd("FCALL")
            .arg("release_lock")
            .arg(0)
            .arg(lock_id)
            .query(&mut self.conn)?;

        Ok(result)
    }

    pub fn lock(
        &mut self,
        resources: &[String],
        expiration: usize,
    ) -> RedisResult<Option<MultiResourceGuard<'_>>> {
        self.acquire(resources, expiration).map(|result| {
            result.map(|lock_id| MultiResourceGuard {
                lock: self,
                lock_id,
            })
        })
    }
}

pub struct MultiResourceGuard<'a> {
    lock: &'a mut MultiResourceLock,
    lock_id: String,
}

impl Drop for MultiResourceGuard<'_> {
    fn drop(&mut self) {
        self.lock.release(&self.lock_id).unwrap();
    }
}
