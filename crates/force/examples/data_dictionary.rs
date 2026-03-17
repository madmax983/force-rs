//! Data Dictionary Generator Example.
//!
//! This example demonstrates how to use the `DataDictionary` generator
//! to create a Markdown document of a Salesforce `SObject`'s schema.

use force::auth::ClientCredentials;
use force::client::ForceClientBuilder;
use force::experimental::DataDictionary;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let client_id =
        std::env::var("SALESFORCE_CLIENT_ID").unwrap_or_else(|_| "dummy_client_id".to_string());
    let client_secret = std::env::var("SALESFORCE_CLIENT_SECRET")
        .unwrap_or_else(|_| "dummy_client_secret".to_string());

    let auth = ClientCredentials::new_sandbox(client_id, client_secret)?;

    let client = ForceClientBuilder::new().authenticate(auth).build().await?;

    let sobject = "Account";
    println!("Generating data dictionary for {sobject}...");

    let dict = DataDictionary::new(&client);
    let md = dict.generate(sobject, true).await?;

    println!("---");
    println!("{md}");
    println!("---");

    Ok(())
}
