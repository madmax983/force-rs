//! Search with SOSL Example
//!
//! Demonstrates searching for records using SOSL.

#[cfg(feature = "rest")]
use anyhow::Context;
#[cfg(feature = "rest")]
use force::api::rest::search::SearchQueryBuilder;
#[cfg(feature = "rest")]
use force::auth::ClientCredentials;
#[cfg(feature = "rest")]
use force::client::{ForceClient, builder};

#[cfg(feature = "rest")]
fn required_env(name: &str) -> anyhow::Result<String> {
    std::env::var(name).with_context(|| format!("{name} environment variable not set"))
}

#[cfg(feature = "rest")]
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

    #[cfg(feature = "rest")]
    {
        let client = build_client().await?;

        let query = SearchQueryBuilder::new()
            .find("Acme")
            .in_all_fields()
            .returning("Account", &["Id", "Name"])
            .returning("Contact", &["Id", "Name", "Email"])
            .limit(5)
            .build();

        let results = client.rest().search(&query).await?;

        println!("Found {} search records", results.search_records.len());
        for record_set in &results.search_records {
            println!(
                "Type: {} (count: {})",
                record_set.attributes.type_,
                record_set.records.len()
            );
        }
    }

    Ok(())
}
