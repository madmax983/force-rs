#[cfg(feature = "rest")]
mod example {
    //! Basic CRUD Operations Example
    //!
    //! Demonstrates create, read, update, and delete flows with the REST API.

    use anyhow::Context;
    use force::auth::ClientCredentials;
    use force::client::{ForceClient, builder};
    use serde_json::json;

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

    #[tokio::main]
    async fn main() -> anyhow::Result<()> {
        tracing_subscriber::fmt::init();

        let client = build_client().await?;

        let create = json!({
            "Name": "Acme Corporation",
            "Industry": "Technology",
            "Website": "https://acme.example.com"
        });

        let created = client.rest().create("Account", &create).await?;
        let account_id = created
            .id
            .ok_or_else(|| anyhow::anyhow!("Account ID should be present"))?;
        println!("Created account: {account_id}");

        let fetched = client.rest().get("Account", &account_id).await?;
        println!(
            "Fetched account: {} ({})",
            fetched["Name"].as_str().unwrap_or("N/A"),
            fetched["Industry"].as_str().unwrap_or("N/A")
        );

        let update = json!({
            "Industry": "Software"
        });
        client
            .rest()
            .update("Account", &account_id, &update)
            .await?;

        let updated = client.rest().get("Account", &account_id).await?;
        println!(
            "Updated industry: {}",
            updated["Industry"].as_str().unwrap_or("N/A")
        );

        client.rest().delete("Account", &account_id).await?;
        println!("Deleted account: {account_id}");

        Ok(())
    }
}

#[cfg(feature = "rest")]
fn main() -> anyhow::Result<()> {
    example::main()
}

#[cfg(not(feature = "rest"))]
fn main() {}
