//! Bulk Query Example
//!
//! This example demonstrates querying large datasets using the Bulk API 2.0.
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
//! cargo run --example bulk_query --features bulk
//! ```

use force::auth::ClientCredentials;
use force::client::builder;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
struct Account {
    #[serde(rename = "Id")]
    id: String,
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Industry")]
    industry: Option<String>,
    #[serde(rename = "Website")]
    website: Option<String>,
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

    // Execute bulk query
    println!("═══ Bulk Query ═══");
    let soql = "SELECT Id, Name, Industry, Website FROM Account WHERE Industry = 'Technology'";
    println!("Query: {}", soql);

    // Creates job, polls until complete, returns streaming results
    let mut stream = client.bulk().bulk_query::<Account>(soql).await?;

    println!("\n═══ Results ═══");
    let mut count = 0;
    while let Some(account) = stream.next().await? {
        count += 1;
        println!(
            "{}. {} ({})",
            count,
            account.name,
            account.industry.unwrap_or_else(|| "N/A".to_string())
        );

        if let Some(website) = &account.website {
            println!("   Website: {}", website);
        }
    }

    println!("\n✓ Retrieved {} records", count);

    Ok(())
}
