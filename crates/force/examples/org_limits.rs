//! Org Limits Example
//!
//! Demonstrates retrieving organization limits.

#[cfg(feature = "rest")]
use anyhow::Context;
#[cfg(feature = "rest")]
use force::auth::ClientCredentials;
#[cfg(feature = "rest")]
use force::client::{builder, ForceClient};

#[cfg(feature = "rest")]
fn required_env(name: &str) -> anyhow::Result<String> {
    std::env::var(name).with_context(|| format!("{name} environment variable not set"))
}

#[cfg(feature = "rest")]
async fn build_client() -> anyhow::Result<ForceClient<ClientCredentials>> {
    let client_id = required_env("SF_CLIENT_ID")?;
    let client_secret = required_env("SF_CLIENT_SECRET")?;

    let auth = ClientCredentials::new(
        client_id,
        client_secret,
        "https://login.salesforce.com/services/oauth2/token",
    );
    builder().authenticate(auth).build().await.map_err(Into::into)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    #[cfg(feature = "rest")]
    {
        let client = build_client().await?;

        let limits = client.rest().limits().await?;

        println!("Org Limits:");
        println!(
            "  Daily API Requests: {}/{}",
            limits.daily_api_requests.remaining, limits.daily_api_requests.max
        );
        println!(
            "  Daily Async Apex Executions: {}/{}",
            limits.daily_async_apex_executions.remaining, limits.daily_async_apex_executions.max
        );
        println!(
            "  Data Storage (MB): {}/{}",
            limits.data_storage_mb.remaining, limits.data_storage_mb.max
        );
        println!(
            "  File Storage (MB): {}/{}",
            limits.file_storage_mb.remaining, limits.file_storage_mb.max
        );
    }

    Ok(())
}
