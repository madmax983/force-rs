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
#[cfg(feature = "bulk")]
mod example {

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

        // Execute bulk query
        println!("═══ Bulk Query ═══");
        let soql = "SELECT Id, Name, Industry, Website FROM Account WHERE Industry = 'Technology'";
        println!("Query: {soql}");

        // Creates job, polls until complete, returns streaming results
        let mut stream = client.bulk().bulk_query::<Account>(soql).await?;

        println!("\n═══ Results ═══");
        let mut count = 0;
        while let Some(account) = stream.next().await? {
            count += 1;
            println!(
                "{}. {} [{}] ({})",
                count,
                account.name,
                account.id,
                account.industry.unwrap_or_else(|| "N/A".to_string())
            );

            if let Some(website) = &account.website {
                println!("   Website: {website}");
            }
        }

        println!("\n✓ Retrieved {count} records");

        Ok(())
    }
}

#[cfg(feature = "bulk")]
fn main() -> anyhow::Result<()> {
    example::main()
}

#[cfg(not(feature = "bulk"))]
fn main() {}
