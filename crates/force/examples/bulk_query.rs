//! Bulk Query Example

#[cfg(feature = "bulk")]
mod example {
    use anyhow::Context;
    use force::auth::ClientCredentials;
    use force::client::{ForceClient, builder};
    use serde::Deserialize;

    #[derive(Deserialize, Debug)]
    struct Account {
        #[serde(rename = "Id")]
        id: String,
        #[serde(rename = "Name")]
        name: String,
        #[serde(rename = "Industry")]
        industry: Option<String>,
    }

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

        let soql = "SELECT Id, Name, Industry FROM Account LIMIT 1000";

        // Returns a stream of Results
        let mut stream = client.bulk().bulk_query::<Account>(soql).await?;

        println!("Query submitted. Streaming results...");

        let mut count = 0;
        while let Some(account) = stream.next().await? {
            count += 1;
            if count <= 5 {
                println!(
                    "Account {}: {} ({:?})",
                    count, account.name, account.industry
                );
            }
        }

        println!("Total records processed: {}", count);

        Ok(())
    }
}

fn main() -> anyhow::Result<()> {
    #[cfg(feature = "bulk")]
    {
        example::main()
    }
    #[cfg(not(feature = "bulk"))]
    {
        println!("This example requires the 'bulk' feature.");
        Ok(())
    }
}
