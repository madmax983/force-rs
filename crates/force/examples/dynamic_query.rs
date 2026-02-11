//! Dynamic Query Example
//!
//! This example demonstrates querying using `DynamicSObject` for flexible data handling.
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
//! cargo run --example dynamic_query --features rest
//! ```

#[cfg(feature = "rest")]
mod example {
    use anyhow::Context;
    use force::auth::ClientCredentials;
    use force::client::builder;
    use force::types::DynamicSObject;

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

        // Execute query returning DynamicSObjects
        println!("═══ Dynamic Query ═══");
        let accounts = client
            .query::<DynamicSObject>("SELECT Id, Name, Industry FROM Account LIMIT 5")
            .await?;

        println!("Found {} accounts:\n", accounts.total_size);

        for record in accounts.records {
            let id = record.get_field_as::<String>("Id")?.unwrap_or_default();
            let name = record
                .get_field_as::<String>("Name")?
                .unwrap_or_else(|| "Unknown".to_string());
            let industry = record
                .get_field_as::<String>("Industry")?
                .unwrap_or_else(|| "None".to_string());

            println!("{id}: {name} ({industry})");
        }

        // Aggregate query example
        println!("\n═══ Aggregate Query ═══");
        let revenue_rows = client
            .query::<DynamicSObject>(
                "SELECT Name, AnnualRevenue FROM Account WHERE AnnualRevenue != null LIMIT 10",
            )
            .await?;

        let total_revenue: f64 = revenue_rows
            .records
            .iter()
            .filter_map(|row| row.get_field_as::<f64>("AnnualRevenue").ok().flatten())
            .sum();

        println!("Total Revenue (from sample): ${total_revenue:.2}");

        Ok(())
    }
}

#[cfg(feature = "rest")]
fn main() -> anyhow::Result<()> {
    example::main()
}

#[cfg(not(feature = "rest"))]
fn main() {
    println!("This example requires the 'rest' feature.");
}
