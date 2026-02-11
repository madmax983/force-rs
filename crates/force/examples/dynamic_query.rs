//! Dynamic Query Example

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
        tracing_subscriber::fmt::init();

        let client_id = required_env("SF_CLIENT_ID")?;
        let client_secret = required_env("SF_CLIENT_SECRET")?;

        let auth = ClientCredentials::new(
            client_id,
            client_secret,
            "https://login.salesforce.com/services/oauth2/token",
        );
        let client = builder().authenticate(auth).build().await?;

        // 1. Basic query with untyped results
        println!("Querying Accounts...");
        let accounts = client
            .query::<DynamicSObject>("SELECT Id, Name, Industry FROM Account LIMIT 5")
            .await?;

        for record in accounts.records {
            let id = record.get_field_as::<String>("Id")?.unwrap_or_default();
            let name = record.get_field_as::<String>("Name")?.unwrap_or_default();
            let industry = record.get_field_as::<String>("Industry")?;

            println!("- {} ({}): {:?}", name, id, industry);
        }

        // 2. Query with manual deserialization for specific fields
        println!("\nCalculating total revenue from top 10 accounts...");
        let revenue_rows = client
            .query::<DynamicSObject>(
                "SELECT AnnualRevenue FROM Account WHERE AnnualRevenue != NULL ORDER BY AnnualRevenue DESC LIMIT 10",
            )
            .await?;

        let total_revenue: f64 = revenue_rows
            .records
            .into_iter()
            .filter_map(|row| row.get_field_as::<f64>("AnnualRevenue").ok().flatten())
            .sum();

        println!("Total Revenue (Top 10): ${total_revenue:.2}");

        Ok(())
    }
}

fn main() -> anyhow::Result<()> {
    #[cfg(feature = "rest")]
    {
        example::main()
    }
    #[cfg(not(feature = "rest"))]
    {
        println!("This example requires the 'rest' feature.");
        Ok(())
    }
}
