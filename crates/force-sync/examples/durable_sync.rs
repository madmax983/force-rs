//! Durable force-sync on autumn-harvest.
//!
//! Run with a Postgres database and Salesforce credentials:
//!
//! ```bash
//! FORCE_SYNC_DATABASE_URL=postgres://postgres:postgres@127.0.0.1:5432/force_sync \
//! SF_CLIENT_ID=xxx SF_CLIENT_SECRET=yyy \
//!   cargo run -p force-sync --example durable_sync --features durable
//! ```
//!
//! The harvest tables go in the same database here. Point
//! `FORCE_SYNC_HARVEST_DATABASE_URL` at another database to separate them.

use deadpool_postgres::{Config as PgConfig, Runtime};
use force::auth::ClientCredentials;
use force::client::builder;
use force_sync::durable::{DurableSync, DurableSyncConfig, harvest_pool, migrate_harvest};
use force_sync::{ObjectSync, PgStore, SyncEngine, migrate};
use tokio_postgres::NoTls;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let sync_url = std::env::var("FORCE_SYNC_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@127.0.0.1:5432/force_sync".to_owned());
    let harvest_url =
        std::env::var("FORCE_SYNC_HARVEST_DATABASE_URL").unwrap_or_else(|_| sync_url.clone());

    let mut config = PgConfig::new();
    config.url = Some(sync_url);
    let pool = config.create_pool(Some(Runtime::Tokio1), NoTls)?;
    migrate(&pool).await?;
    migrate_harvest(&harvest_url).await?;

    let auth = ClientCredentials::new_production(
        std::env::var("SF_CLIENT_ID")?,
        std::env::var("SF_CLIENT_SECRET")?,
    );
    let client = builder().authenticate(auth).build().await?;
    let engine = SyncEngine::builder(client)
        .postgres(PgStore::new(pool))
        .object(ObjectSync::new("Account").external_id("External_Id__c"))
        .build()?;

    let runtime = DurableSync::new(engine, harvest_pool(&harvest_url, 16)?)
        .with_config(DurableSyncConfig::default())?
        .start()?;
    println!("durable sync running; Ctrl-C drains the worker and stops");

    tokio::signal::ctrl_c().await?;
    runtime.stop().await?;
    Ok(())
}
