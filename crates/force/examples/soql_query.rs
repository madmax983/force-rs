//! Typed SOQL Query Example

#[cfg(feature = "rest")]
mod example {
    use anyhow::Context;
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
    }

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

        let query = "SELECT Id, Name, Industry FROM Account LIMIT 5";
        println!("Executing query: {}", query);

        let accounts = client.query::<Account>(query).await?;

        println!("Found {} accounts:", accounts.len());
        for account in accounts {
            println!(
                "- {} ({}) - Industry: {:?}",
                account.name, account.id, account.industry
            );
        }

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
