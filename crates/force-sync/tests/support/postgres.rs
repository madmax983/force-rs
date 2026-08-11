//! `Postgres` test helpers for force-sync integration tests.

use std::env;

use deadpool_postgres::{Config, Pool, Runtime};
use tokio_postgres::NoTls;

/// Builds a `Postgres` pool from `FORCE_SYNC_TEST_DATABASE_URL`.
#[must_use]
pub fn test_pool() -> Pool {
    let url = env::var("FORCE_SYNC_TEST_DATABASE_URL").unwrap_or_else(|_| {
        panic!("FORCE_SYNC_TEST_DATABASE_URL must be set for force-sync PostgreSQL tests")
    });
    let mut config = Config::new();
    config.url = Some(url);

    config
        .create_pool(Some(Runtime::Tokio1), NoTls)
        .unwrap_or_else(|err| {
            panic!("failed to create PostgreSQL pool for force-sync tests: {err}")
        })
}

/// Drops and recreates the `public` schema for a clean test environment.
pub async fn reset_schema(pool: &Pool) -> force_sync::Result<()> {
    let client = pool.get().await?;
    client
        .batch_execute("drop schema if exists public cascade; create schema public;")
        .await?;
    Ok(())
}
