//! Bulk Update Example

#[cfg(feature = "bulk")]
mod example {
    use anyhow::Context;
    use force::api::bulk::ingest::IngestJobBuilder;
    use force::api::bulk::types::JobOperation;
    use force::auth::ClientCredentials;
    use force::client::{ForceClient, builder};
    use serde::{Deserialize, Serialize};

    #[derive(Deserialize, Debug)]
    struct Account {
        #[serde(rename = "Id")]
        id: String,
        #[serde(rename = "Name")]
        name: String,
    }

    #[derive(Serialize, Debug)]
    struct AccountUpdate {
        #[serde(rename = "Id")]
        id: String,
        #[serde(rename = "Description")]
        description: String,
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

        // 1. Get accounts to update
        let soql = "SELECT Id, Name FROM Account LIMIT 5";
        let mut stream = client.bulk().bulk_query::<Account>(soql).await?;

        let mut accounts = Vec::new();
        while let Some(mut account) = stream.next().await? {
            accounts.push(AccountUpdate {
                id: account.id,
                description: format!("Updated via Bulk API: {}", account.name),
            });
        }

        if accounts.is_empty() {
            println!("No accounts found");
            return Ok(());
        }

        // 2. Perform update
        let job_info = client.bulk().bulk_update("Account", &accounts).await?;
        println!(
            "Update job completed: {} (processed: {}, failed: {})",
            job_info.id, job_info.number_records_processed, job_info.number_records_failed
        );

        // 3. Alternative manual approach
        let job = IngestJobBuilder::new("Account", JobOperation::Update)
            .build(&client.bulk())
            .await?;

        let mut wtr = csv::Writer::from_writer(vec![]);
        for acc in &accounts {
            wtr.serialize(acc)?;
        }
        let data = wtr.into_inner()?;

        job.upload(&data).await?.close().await?;

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
