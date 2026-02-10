#[cfg(feature = "bulk")]
mod example {
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

    use force::auth::ClientCredentials;
    use force::client::builder;
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

    use anyhow::Context;

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
        let job_info = client.bulk().bulk_insert("Account", &accounts).await?;

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
fn main() -> anyhow::Result<()> {
    example::main()
}

#[cfg(not(feature = "bulk"))]
fn main() {}
