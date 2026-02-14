//! Dynamic Query Example
//!
//! Demonstrates querying dynamic records with `DynamicSObject`.

#[cfg(feature = "rest")]
mod example {
    use anyhow::Context;
    use force::auth::ClientCredentials;
    use force::client::{ForceClient, builder};
    use force::types::DynamicSObject;

    fn required_env(name: &str) -> anyhow::Result<String> {
        std::env::var(name).with_context(|| format!("{name} environment variable not set"))
    }

    async fn build_client() -> anyhow::Result<ForceClient<ClientCredentials>> {
        let client_id = required_env("SF_CLIENT_ID")?;
        let client_secret = required_env("SF_CLIENT_SECRET")?;

        let auth = ClientCredentials::new(
            client_id,
            client_secret,
            "https://login.salesforce.com/services/oauth2/token",
        );
        builder().build(auth)
            .await
            .map_err(Into::into)
    }

    pub async fn main() -> anyhow::Result<()> {
        tracing_subscriber::fmt::init();
        let client = build_client().await?;

        let accounts = client
            .query::<DynamicSObject>("SELECT Id, Name, Industry FROM Account LIMIT 5")
            .await?;

        println!("Accounts:");
        for record in &accounts.records {
            let id = record.get_field_as::<String>("Id")?.unwrap_or_default();
            let name = record.get_field_as::<String>("Name")?.unwrap_or_default();
            let industry = record
                .get_field_as::<String>("Industry")?
                .unwrap_or_else(|| "(none)".to_string());
            println!("- {name} ({id}) [{industry}]");
        }

        let revenue_rows = client
            .query::<DynamicSObject>(
                "SELECT Name, AnnualRevenue FROM Account WHERE AnnualRevenue != null LIMIT 10",
            )
            .await?;

        let revenues: Vec<f64> = revenue_rows
            .records
            .iter()
            .filter_map(|row| row.get_field_as::<f64>("AnnualRevenue").ok().flatten())
            .collect();

        if !revenues.is_empty() {
            let total: f64 = revenues.iter().sum();
            let count_u32 =
                u32::try_from(revenues.len()).context("too many revenues to aggregate")?;
            let average = total / f64::from(count_u32);
            println!("\nRevenue rows: {}", revenues.len());
            println!("Total revenue: ${total:.2}");
            println!("Average revenue: ${average:.2}");
        }

        Ok(())
    }
}

#[cfg(feature = "rest")]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    example::main().await
}

#[cfg(not(feature = "rest"))]
fn main() {
    println!("This example requires the 'rest' feature.");
}
