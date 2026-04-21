//! Data Dictionary Generator Example.
//!
//! This example demonstrates how to use the `DataDictionary` generator
//! to create a Markdown document of a Salesforce `SObject`'s schema.

use force::auth::ClientCredentials;
use force::client::ForceClientBuilder;
use force::schema::DataDictionary;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let client_id =
        std::env::var("SALESFORCE_CLIENT_ID").unwrap_or_else(|_| "dummy_client_id".to_string());
    let client_secret = std::env::var("SALESFORCE_CLIENT_SECRET")
        .unwrap_or_else(|_| "dummy_client_secret".to_string());
    let my_domain_url = std::env::var("SALESFORCE_MY_DOMAIN_URL")
        .unwrap_or_else(|_| "https://your-org.my.salesforce.com".to_string());

    let auth = ClientCredentials::new_my_domain(client_id, client_secret, my_domain_url);

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
