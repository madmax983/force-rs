//! SOSL Search Example
//!
//! This example demonstrates using SOSL (Salesforce Object Search Language) to search for records.
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
//! cargo run --example search --features rest
//! ```

#[cfg(feature = "rest")]
mod example {
    use anyhow::Context;
    use force::auth::ClientCredentials;
    use force::client::builder;

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

        // Execute search
        let sosl = "FIND {Acme} IN ALL FIELDS RETURNING Account(Id, Name, Industry), Contact(FirstName, LastName)";
        println!("═══ SOSL Search ═══");
        println!("Query: {sosl}");

        let result = client.rest().search(sosl).await?;

        println!(
            "\nFound {} records across {} objects.",
            result.search_records.len(),
            result
                .search_records
                .iter()
                .map(|r| r.attributes.type_.clone())
                .collect::<std::collections::HashSet<_>>()
                .len()
        );

        for group in result.search_records {
            let type_ = &group.attributes.type_;
            for record in group.records {
                let id = record.get("Id").and_then(|v| v.as_str()).unwrap_or("N/A");
                let name = record.get("Name").and_then(|v| v.as_str()).unwrap_or("N/A");

                println!("[{type_}] {id}: {name}");
            }
        }

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
