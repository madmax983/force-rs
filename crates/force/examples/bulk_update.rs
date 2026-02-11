//! Bulk Update Example
//!
//! This example demonstrates updating records in bulk using the Bulk API 2.0.
//!
//! # Setup
//!
//! Set the following environment variables:
//! - `SF_CLIENT_ID` - OAuth client ID
//! - `SF_CLIENT_SECRET` - OAuth client secret
//!
//! # Run
//!
//! ```bash
//! cargo run --example bulk_update --features bulk
//! ```

#[cfg(feature = "bulk")]
use force::auth::ClientCredentials;
#[cfg(feature = "bulk")]
use force::client::builder;
#[cfg(feature = "bulk")]
use serde::{Deserialize, Serialize};

#[cfg(feature = "bulk")]
#[derive(Serialize, Deserialize)]
struct Account {
    #[serde(rename = "Id")]
    id: String,
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Industry")]
    #[serde(skip_serializing_if = "Option::is_none")]
    industry: Option<String>,
}

#[cfg(feature = "bulk")]
use anyhow::Context;

#[cfg(feature = "bulk")]
fn required_env(name: &str) -> anyhow::Result<String> {
    std::env::var(name).with_context(|| format!("{name} environment variable not set"))
}
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    #[cfg(feature = "bulk")]
    {
        // Get credentials from environment
        let client_id = required_env("SF_CLIENT_ID")?;
        let client_secret = required_env("SF_CLIENT_SECRET")?;

        println!("═══ Authenticating ═══");
        let auth = ClientCredentials::new(
            client_id,
            client_secret,
            "https://login.salesforce.com/services/oauth2/token",
        );
        let client = builder().authenticate(auth).build().await?;
        println!("✓ Authentication successful\n");

        // Query for Technology accounts to update
        println!("═══ Querying Accounts ═══");
        let soql = "SELECT Id, Name, Industry FROM Account WHERE Industry = 'Technology' LIMIT 10";
        // Explicit type annotation to fix E0282
        let mut stream: force::api::bulk::query::BulkQueryStream<Account, ClientCredentials> =
            client.bulk().bulk_query::<Account>(soql).await?;

        let mut accounts = Vec::new();
        while let Some(mut account) = stream.next().await? {
            println!("Found: {} ({})", account.name, account.id);
            // Update the industry field
            account.industry = Some("Software".to_string());
            accounts.push(account);
        }

        if accounts.is_empty() {
            println!("\nNo Technology accounts found to update");
            return Ok(());
        }

        // Perform bulk update (creates job, uploads CSV, closes, and polls)
        println!("\n═══ Bulk Update ═══");
        println!("Updating {} accounts...", accounts.len());
        let job_info = client.bulk().bulk_update("Account", &accounts).await?;

        println!("\n═══ Results ═══");
        println!("Job ID: {}", job_info.id);
        println!("State: {:?}", job_info.state);
        println!(
            "Records Processed: {}",
            job_info.number_records_processed.unwrap_or(0)
        );
        println!(
            "Records Failed: {}",
            job_info.number_records_failed.unwrap_or(0)
        );

        if job_info.number_records_failed.unwrap_or(0) == 0 {
            println!("\n✓ All records updated successfully!");
        } else {
            println!("\n⚠ Some records failed - check Salesforce logs");
        }
    }

    Ok(())
}
