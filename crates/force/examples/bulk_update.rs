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
mod example {
    use anyhow::Context;
    use force::auth::ClientCredentials;
    use force::client::builder;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Debug)]
    struct Account {
        #[serde(rename = "Id")]
        id: String,
        #[serde(rename = "Name")]
        name: String,
        #[serde(rename = "Description")]
        description: String,
    }

    fn required_env(name: &str) -> anyhow::Result<String> {
        std::env::var(name).with_context(|| format!("{name} environment variable not set"))
    }

    #[tokio::main]
    pub async fn main() -> anyhow::Result<()> {
        // Initialize tracing
        tracing_subscriber::fmt::init();

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

        // 1. Query some records to update
        println!("═══ Querying Accounts ═══");
        let soql = "SELECT Id, Name FROM Account LIMIT 5";
        let mut stream = client.bulk().bulk_query::<Account>(soql).await?;

        let mut accounts = Vec::new();
        while let Some(mut account) = stream.next().await? {
            // Modify the records
            account.description = format!("Updated by force-rs at {}", chrono::Utc::now());
            accounts.push(account);
        }

        if accounts.is_empty() {
            println!("No accounts found to update.");
            return Ok(());
        }

        println!("Found {} accounts to update.", accounts.len());

        // 2. Perform bulk update
        println!("\n═══ Bulk Update ═══");
        let job_info = client.bulk().bulk_update("Account", &accounts).await?;

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
}

#[cfg(feature = "bulk")]
fn main() -> anyhow::Result<()> {
    example::main()
}

#[cfg(not(feature = "bulk"))]
fn main() {
    println!("This example requires the 'bulk' feature.");
}
