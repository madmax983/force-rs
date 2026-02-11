//! Bulk Insert Example

#[cfg(feature = "bulk")]
mod example {
    use anyhow::Context;
    use force::api::bulk::ingest::IngestJobBuilder;
    use force::api::bulk::types::JobOperation;
    use force::auth::ClientCredentials;
    use force::client::{ForceClient, builder};
    use serde::Serialize;

    #[derive(Serialize, Debug)]
    struct Account {
        #[serde(rename = "Name")]
        name: String,
        #[serde(rename = "Industry")]
        industry: String,
        #[serde(rename = "AnnualRevenue")]
        revenue: f64,
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

        // 1. Prepare data
        let accounts = vec![
            Account {
                name: "Bulk Corp 1".to_string(),
                industry: "Technology".to_string(),
                revenue: 1_000_000.0,
            },
            Account {
                name: "Bulk Corp 2".to_string(),
                industry: "Finance".to_string(),
                revenue: 5_000_000.0,
            },
        ];

        // 2. Create job using helper
        // This handles CSV serialization, job creation, upload, closing, and waiting
        let job_info = client.bulk().bulk_insert("Account", &accounts).await?;

        println!(
            "Job completed: {} (processed: {}, failed: {})",
            job_info.id,
            job_info.number_records_processed.unwrap_or(0),
            job_info.number_records_failed.unwrap_or(0)
        );

        // 3. Alternatively, manual control:
        let job = IngestJobBuilder::new("Account", JobOperation::Insert)
            .build(&client.bulk())
            .await?;

        // Serialize manually if needed...
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
