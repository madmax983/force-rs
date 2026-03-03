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
    use force::client::ForceClientBuilder;
    use serde::{Deserialize, Serialize};

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
        // Use new_production() for standard login URL
        // For Sandbox, use: ClientCredentials::new_sandbox("client-id", "client-secret")
        let auth = ClientCredentials::new_production(client_id, client_secret);
        let client = ForceClientBuilder::new().authenticate(auth).build().await?;
        println!("✓ Authentication successful\n");

        // Query for Technology accounts to update
        println!("═══ Querying Accounts ═══");
        let soql = "SELECT Id, Name, Industry FROM Account WHERE Industry = 'Technology' LIMIT 10";
        let mut stream = client.bulk().query::<Account>(soql).await?;

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
        let job_info = client.bulk().update("Account", &accounts).await?;

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
