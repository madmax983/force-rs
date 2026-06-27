//! Example showing how to generate an Elasticsearch mapping from a Salesforce schema.

use force::api::RestOperation;
use force::auth::ClientCredentials;
use force::client::ForceClientBuilder;
use force::schema::generate_elasticsearch_mapping;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Authenticate with OAuth 2.0 client credentials
    let auth = ClientCredentials::new_my_domain(
        "your-client-id",
        "your-client-secret",
        "https://your-org.my.salesforce.com",
    );

    let client = ForceClientBuilder::new().authenticate(auth).build().await?;

    let describe = client.rest().describe("Account").await?;
    let mapping = generate_elasticsearch_mapping(&describe);

    println!("{}", serde_json::to_string_pretty(&mapping)?);

    Ok(())
}
