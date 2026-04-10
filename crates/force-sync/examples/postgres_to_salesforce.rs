//! Compile-checked vertical slice for `force-sync`.

use async_trait::async_trait;
use deadpool_postgres::{Config as PgConfig, Pool, Runtime};
use force::auth::{AccessToken, Authenticator, TokenResponse};
use force::client::builder;
use force_sync::{ObjectSync, PgStore, SyncEngine, migrate};
use tokio_postgres::NoTls;

#[derive(Debug, Clone)]
struct ExampleAuthenticator;

#[async_trait]
impl Authenticator for ExampleAuthenticator {
    async fn authenticate(&self) -> force::error::Result<AccessToken> {
        Ok(AccessToken::from_response(TokenResponse {
            access_token: "example-access-token".to_owned(),
            instance_url: "https://example.my.salesforce.invalid".to_owned(),
            token_type: "Bearer".to_owned(),
            issued_at: "0".to_owned(),
            signature: "example-signature".to_owned(),
            expires_in: Some(3600),
            refresh_token: None,
        }))
    }

    async fn refresh(&self) -> force::error::Result<AccessToken> {
        self.authenticate().await
    }
}

fn postgres_pool() -> anyhow::Result<Pool> {
    let url = std::env::var("FORCE_SYNC_TEST_DATABASE_URL").unwrap_or_else(|_| {
        "postgres://postgres:postgres@127.0.0.1:5432/force_sync_test".to_owned()
    });
    let mut config = PgConfig::new();
    config.url = Some(url);

    Ok(config.create_pool(Some(Runtime::Tokio1), NoTls)?)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let pool = postgres_pool()?;
    migrate(&pool).await?;

    let client = builder().authenticate(ExampleAuthenticator).build().await?;

    let engine = SyncEngine::builder(client)
        .postgres(PgStore::new(pool))
        .object(ObjectSync::new("Account").external_id("External_Id__c"))
        .reconcile_batch_size(10)
        .build()?;

    let _ = engine.run_capture_postgres_once().await?;
    let _ = engine.run_apply_once().await?;
    let _ = engine.run_reconcile_once().await?;

    Ok(())
}
