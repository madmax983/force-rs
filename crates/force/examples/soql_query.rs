//! SOQL Query Example
//!
//! This example demonstrates standard SOQL queries, pagination, and relationship traversal.
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
//! cargo run --example soql_query --features rest
//! ```

#[cfg(feature = "rest")]
mod example {
    use anyhow::Context;
    use force::auth::ClientCredentials;
    use force::client::builder;
    use serde::Deserialize;

    #[derive(Deserialize, Debug)]
    struct Account {
        #[serde(rename = "Id")]
        _id: String,
        #[serde(rename = "Name")]
        name: String,
        #[serde(rename = "Industry")]
        industry: Option<String>,
    }

    #[derive(Deserialize, Debug)]
    struct Contact {
        #[serde(rename = "Id")]
        _id: String,
        #[serde(rename = "LastName")]
        last_name: String,
        #[serde(rename = "Email")]
        email: Option<String>,
    }

    #[derive(Deserialize, Debug)]
    struct IndustryStats {
        #[serde(rename = "Industry")]
        industry: Option<String>,
        #[serde(rename = "expr0")]
        count: i64,
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

        // 1. Basic Query
        println!("═══ Basic Account Query ═══");
        let accounts = client
            .query::<Account>("SELECT Id, Name, Industry FROM Account ORDER BY Name LIMIT 10")
            .await?;

        for acc in &accounts.records {
            println!("- {} ({})", acc.name, acc.industry.as_deref().unwrap_or("None"));
        }
        println!("Total Size: {}", accounts.total_size);

        // 2. Pagination (Manual)
        println!("\n═══ Contact Query (Pagination) ═══");
        let mut contacts = client
            .query::<Contact>("SELECT Id, LastName, Email FROM Contact ORDER BY LastName LIMIT 5")
            .await?;

        print_contacts(&contacts.records);

        while let Some(next) = &contacts.next_records_url {
            println!("... fetching next page ...");
            contacts = client.query_more::<Contact>(next).await?;
            print_contacts(&contacts.records);
        }

        // 3. Aggregate Query
        println!("\n═══ Industry Stats (Aggregate) ═══");
        let stats = client
            .query::<IndustryStats>(
                "SELECT Industry, COUNT(Id) FROM Account GROUP BY Industry HAVING COUNT(Id) > 0",
            )
            .await?;

        for stat in stats.records {
            println!(
                "{}: {}",
                stat.industry.as_deref().unwrap_or("Unknown"),
                stat.count
            );
        }

        Ok(())
    }

    fn print_contacts(contacts: &[Contact]) {
        for c in contacts {
            println!(
                "{} <{}>",
                c.last_name,
                c.email.as_deref().unwrap_or("no email")
            );
        }
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
