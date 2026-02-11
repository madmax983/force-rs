//! Describe `SObject` Example
//!
//! Demonstrates retrieving metadata for `SObjects`.

#[cfg(feature = "rest")]
use anyhow::Context;
#[cfg(feature = "rest")]
use force::auth::ClientCredentials;
#[cfg(feature = "rest")]
use force::client::{ForceClient, builder};

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
    builder()
        .authenticate(auth)
        .build()
        .await
        .map_err(Into::into)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    #[cfg(feature = "rest")]
    {
        let client = build_client().await?;

        // Describe Global
        let global = client.rest().describe_global().await?;
        println!("Global Describe:");
        println!("  Encoding: {}", global.encoding);
        println!("  Max Batch Size: {}", global.max_batch_size);
        println!("  SObjects count: {}", global.sobjects.len());

        if let Some(account_meta) = global.sobjects.iter().find(|s| s.name == "Account") {
            println!("  Account label: {}", account_meta.label);
            println!("  Account key prefix: {:?}", account_meta.key_prefix);
        }

        // Describe SObject
        let describe = client.rest().describe("Account").await?;
        println!("\nAccount Describe:");
        println!("  Name: {}", describe.name);
        println!("  Label: {}", describe.label);
        println!("  Custom: {}", describe.custom);
        println!("  Fields count: {}", describe.fields.len());

        println!("\n  First 5 fields:");
        for field in describe.fields.iter().take(5) {
            println!("    - {} ({:?})", field.name, field.type_);
        }
    }

    Ok(())
}
