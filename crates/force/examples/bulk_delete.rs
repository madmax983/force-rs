//! Bulk Delete Example

#[cfg(feature = "bulk")]
mod example {
    use anyhow::Context;
    use force::api::bulk::ingest::IngestJobBuilder;
    use force::api::bulk::types::JobOperation;
    use force::auth::ClientCredentials;
    use force::client::{ForceClient, builder};
    use serde::Deserialize;

    #[derive(Deserialize, Debug)]
    struct Account {
        #[serde(rename = "Id")]
        id: String,
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

        // 1. Query for accounts to delete (careful!)
        let soql = "SELECT Id FROM Account WHERE Name LIKE 'To Delete%' LIMIT 10";
        let mut stream = client.bulk().bulk_query::<Account>(soql).await?;

        let mut account_ids = Vec::new();
        while let Some(account) = stream.next().await? {
            account_ids.push(account.id);
        }

        if account_ids.is_empty() {
            println!("No accounts found to delete");
            return Ok(());
        }

        println!("Found {} accounts to delete", account_ids.len());

        // 2. Format IDs as CSV (header: "Id")
        let mut csv = String::from("Id\n");
        for id in &account_ids {
            csv.push_str(id);
            csv.push('\n');
        }

        // 3. Create and execute delete job
        let job_info = client.bulk().bulk_delete("Account", &account_ids).await?;
        println!("Delete job submitted: {}", job_info.id);

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
