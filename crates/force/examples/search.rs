//! SOSL Search Example
//!
//! Demonstrates SOSL searches across multiple object types.

#[cfg(feature = "rest")]
mod example {
    use anyhow::Context;
    use force::auth::ClientCredentials;
    use force::client::{ForceClient, builder};

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
        builder()
            .authenticate(auth)
            .build()
            .await
            .map_err(Into::into)
    }

    async fn run_search(
        client: &ForceClient<ClientCredentials>,
        sosl: &str,
        label: &str,
    ) -> anyhow::Result<()> {
        println!("\n=== {label} ===");
        let result = client.rest().search(sosl).await?;

        for bucket in &result.search_records {
            println!(
                "{}: {} results",
                bucket.attributes.type_,
                bucket.records.len()
            );
            for record in &bucket.records {
                let id = record.get("Id").and_then(|v| v.as_str()).unwrap_or("N/A");
                let name = record.get("Name").and_then(|v| v.as_str()).unwrap_or("N/A");
                println!("  - {name} ({id})");
            }
        }

        Ok(())
    }

    pub async fn main() -> anyhow::Result<()> {
        tracing_subscriber::fmt::init();
        let client = build_client().await?;

        run_search(
            &client,
            "FIND {John} IN NAME FIELDS RETURNING Account(Id, Name), Contact(Id, Name)",
            "Name Search",
        )
        .await?;

        run_search(
            &client,
            "FIND {example.com} IN EMAIL FIELDS RETURNING Contact(Id, Name, Email)",
            "Email Search",
        )
        .await?;

        run_search(
            &client,
            "FIND {Acme*} IN NAME FIELDS RETURNING Account(Id, Name)",
            "Wildcard Search",
        )
        .await?;

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
