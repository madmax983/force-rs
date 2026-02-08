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

use force::auth::ClientCredentials;
use force::client::builder;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct AccountUpdate {
    #[serde(rename = "Id")]
    id: String,
    #[serde(rename = "Industry")]
    industry: String,
}

#[derive(Deserialize)]
struct Account {
    #[serde(rename = "Id")]
    id: String,
    #[serde(rename = "Name")]
    name: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Get credentials from environment
    let client_id =
        std::env::var("SF_CLIENT_ID").expect("SF_CLIENT_ID environment variable not set");
    let client_secret =
        std::env::var("SF_CLIENT_SECRET").expect("SF_CLIENT_SECRET environment variable not set");

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
    let soql = "SELECT Id, Name FROM Account WHERE Industry = 'Technology' LIMIT 10";
    let mut stream = client.bulk().bulk_query::<Account>(soql).await?;

    let mut account_ids = Vec::new();
    while let Some(account) = stream.next().await? {
        println!("Found: {} ({})", account.name, account.id);
        account_ids.push(account.id);
    }

    if account_ids.is_empty() {
        println!("\nNo Technology accounts found to update");
        return Ok(());
    }

    // Prepare bulk update data
    println!("\n═══ Bulk Update ═══");
    let updates: Vec<AccountUpdate> = account_ids
        .iter()
        .map(|id| AccountUpdate {
            id: id.clone(),
            industry: "Software".to_string(),
        })
        .collect();

    println!("Updating {} accounts...", updates.len());

    // Perform bulk update (creates job, uploads CSV, closes, and polls)
    let job_info = client.bulk().bulk_update("Account", &updates).await?;

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

    Ok(())
}
