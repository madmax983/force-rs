//! Example showing how to generate a CSV data dictionary.
use force::auth::ClientCredentials;
use force::client::ForceClientBuilder;
use force::schema::CsvDictionary;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client_id = std::env::var("SF_CLIENT_ID").unwrap_or_default();
    let client_secret = std::env::var("SF_CLIENT_SECRET").unwrap_or_default();
    let my_domain_url = std::env::var("SF_MY_DOMAIN_URL")
        .unwrap_or_else(|_| "https://test.salesforce.com".to_string());
    let auth = ClientCredentials::new_my_domain(client_id, client_secret, my_domain_url);
    let client = ForceClientBuilder::new().authenticate(auth).build().await?;
    let dict = CsvDictionary::new(&client);
    let csv = dict.generate("Account").await?;
    println!("{csv}");
    Ok(())
}
