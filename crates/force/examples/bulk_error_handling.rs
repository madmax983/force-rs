//! Bulk API Error Handling Example

#[cfg(feature = "bulk")]
use anyhow::Context;
#[cfg(feature = "bulk")]
use force::api::bulk::csv::{deserialize_from_csv, serialize_to_csv};
#[cfg(feature = "bulk")]
use force::api::bulk::ingest::IngestJobBuilder;
#[cfg(feature = "bulk")]
use force::api::bulk::types::JobOperation;
#[cfg(feature = "bulk")]
use force::auth::ClientCredentials;
#[cfg(feature = "bulk")]
use force::client::{ForceClient, builder};
#[cfg(feature = "bulk")]
use serde::{Deserialize, Serialize};

#[cfg(feature = "bulk")]
#[derive(Serialize, Deserialize, Debug)]
struct Account {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Industry", skip_serializing_if = "Option::is_none")]
    industry: Option<String>,
}

#[cfg(feature = "bulk")]
#[derive(Deserialize, Debug)]
struct FailedRecord {
    #[serde(rename = "Name", default)]
    name: String,
    #[serde(rename = "sf__Error")]
    error: String,
    #[serde(rename = "sf__Id", default)]
    id: String,
}

#[cfg(feature = "bulk")]
#[derive(Deserialize, Debug)]
struct SuccessRecord {
    #[serde(rename = "sf__Id")]
    id: String,
    #[serde(rename = "sf__Created")]
    created: String,
}

#[cfg(feature = "bulk")]
fn required_env(name: &str) -> anyhow::Result<String> {
    std::env::var(name).with_context(|| format!("{name} environment variable not set"))
}

#[cfg(feature = "bulk")]
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

    #[cfg(feature = "bulk")]
    {
        let client = build_client().await?;

        let accounts = vec![
            Account {
                name: "Valid Corp".to_string(),
                industry: Some("Technology".to_string()),
            },
            Account {
                name: String::new(),
                industry: Some("Finance".to_string()),
            },
        ];

        let mut csv = Vec::new();
        serialize_to_csv(&accounts, &mut csv)?;

        // Split chain to help type inference and handle results correctly
        let job_builder = IngestJobBuilder::new("Account", JobOperation::Insert);
        let job = job_builder.build(&client.bulk()).await?;
        let job = job.upload(&csv).await?;
        let job = job.close().await?;

        // Poll until complete returns IngestJob<JobComplete>, which we need to capture
        let job = job.poll_until_complete().await?;

        let successful_csv = job.successful_results().await?;
        let successful: Vec<SuccessRecord> = deserialize_from_csv(&successful_csv[..])?;
        for record in &successful {
            println!("Created: {} (created flag: {})", record.id, record.created);
        }

        let failed_csv = job.failed_results().await?;
        let failed: Vec<FailedRecord> = deserialize_from_csv(&failed_csv[..])?;
        for record in &failed {
            println!(
                "Failed: {} (id: {}, name: {})",
                record.error, record.id, record.name
            );
        }

        let invalid_soql = "SELECT InvalidField__c FROM Account";
        match client.bulk().bulk_query::<Account>(invalid_soql).await {
            Ok(_) => println!("Unexpectedly created query job"),
            Err(err) => println!("Expected query error: {err}"),
        }
    }

    Ok(())
}
