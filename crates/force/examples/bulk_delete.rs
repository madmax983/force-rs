//! Bulk Delete Example
//!
//! This example demonstrates deleting records in bulk using the Bulk API 2.0.
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
//! cargo run --example bulk_delete --features bulk
//! ```

#[cfg(feature = "bulk")]
mod example {
    use anyhow::Context;
    use force::auth::ClientCredentials;
    use force::client::builder;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct Account {
        #[serde(rename = "Id")]
        id: String,
        #[serde(rename = "Name")]
        name: String,
    }

    fn required_env(name: &str) -> anyhow::Result<String> {
        std::env::var(name).with_context(|| format!("{name} environment variable not set"))
    }

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

        // Query for test accounts to delete
        println!("═══ Querying Accounts ═══");
        let soql = "SELECT Id, Name FROM Account WHERE Name LIKE 'Test%' LIMIT 10";
        let mut stream = client.bulk().bulk_query::<Account>(soql).await?;

        let mut account_ids = Vec::new();
        while let Some(account) = stream.next().await? {
            println!("Found: {} ({})", account.name, account.id);
            account_ids.push(account.id);
        }

        if account_ids.is_empty() {
            println!("\nNo test accounts found to delete");
            return Ok(());
        }

        // Confirm deletion
        println!("\n⚠ About to delete {} accounts!", account_ids.len());
        println!("Press Ctrl+C to cancel, or Enter to continue...");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;

        // Perform bulk delete
        println!("\n═══ Bulk Delete ═══");
        println!("Deleting {} accounts...", account_ids.len());

        // Perform bulk delete (creates job, uploads IDs as CSV, closes, and polls)
        let job_info = client.bulk().bulk_delete("Account", &account_ids).await?;

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
            println!("\n✓ All records deleted successfully!");
        } else {
            println!("\n⚠ Some records failed - check Salesforce logs");
        }

        Ok(())
    }
}

#[cfg(feature = "bulk")]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    example::main().await
}

#[cfg(not(feature = "bulk"))]
fn main() {
    println!("This example requires the 'bulk' feature.");
}
