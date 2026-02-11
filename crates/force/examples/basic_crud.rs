//! Basic CRUD Example

#[cfg(feature = "rest")]
mod example {
    use anyhow::Context;
    use force::auth::ClientCredentials;
    use force::client::{builder, ForceClient};
    use force::types::DynamicSObject;
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
    pub async fn main() -> anyhow::Result<()> {
        tracing_subscriber::fmt::init();
        let client = build_client().await?;

        let create = json!({
            "Name": "New Account"
        });
        let created = client.rest().create("Account", &create).await?;
        let account_id = created.id;
        println!("Created: {}", account_id);

        let fetched = client.rest().get("Account", &account_id).await?;
        let sobject: DynamicSObject = fetched.json()?;
        println!(
            "Fetched: {}",
            sobject
                .get_field("Name")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown")
        );

        let update = json!({
            "Name": "Updated Account"
        });
        client
            .rest()
            .update("Account", &account_id, &update)
            .await?;
        let updated = client.rest().get("Account", &account_id).await?;
        let updated_sobject: DynamicSObject = updated.json()?;
        println!(
            "Updated: {}",
            updated_sobject
                .get_field("Name")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown")
        );

        client.rest().delete("Account", &account_id).await?;
        println!("Deleted: {}", account_id);

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
