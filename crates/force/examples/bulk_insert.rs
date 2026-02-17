//! Bulk Insert Example
//!
//! This example demonstrates inserting records in bulk using the Bulk API 2.0.
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
//! cargo run --example bulk_insert --features bulk
//! ```

#[cfg(feature = "bulk")]
mod example {
    use anyhow::Context;
    use force::auth::ClientCredentials;
    use force::client::ForceClientBuilder;
    use serde::Serialize;

    #[derive(Serialize)]
    struct Account {
        #[serde(rename = "Name")]
        name: String,
        #[serde(rename = "Industry")]
        industry: String,
        #[serde(rename = "Website")]
        website: String,
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
        let auth = ClientCredentials::new_production(client_id, client_secret);
        let client = ForceClientBuilder::new().authenticate(auth).build().await?;
        println!("✓ Authentication successful\n");

        // Prepare bulk data
        println!("═══ Bulk Insert ═══");
        let accounts = vec![
            Account {
                name: "Acme Corporation".to_string(),
                industry: "Technology".to_string(),
                website: "https://acme.example.com".to_string(),
            },
            Account {
                name: "Global Industries".to_string(),
                industry: "Manufacturing".to_string(),
                website: "https://global.example.com".to_string(),
            },
            Account {
                name: "Tech Solutions".to_string(),
                industry: "Technology".to_string(),
                website: "https://techsol.example.com".to_string(),
            },
        ];

        println!("Inserting {} accounts...", accounts.len());

        // Perform bulk insert (creates job, uploads CSV, closes, and polls)
        let job_info = client.bulk().insert("Account", &accounts).await?;

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
            println!("\n✓ All records inserted successfully!");
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
