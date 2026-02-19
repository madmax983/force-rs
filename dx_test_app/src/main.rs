use force::prelude::*;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Account {
    #[serde(rename = "Id")]
    id: String,
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Industry")]
    industry: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Authenticate with OAuth 2.0 client credentials
    let auth = ClientCredentials::new_production(
        "your-client-id",
        "your-client-secret",
    );

    let client = ForceClientBuilder::new()
        .authenticate(auth)
        .build()
        .await?;

    // Execute typed SOQL query
    let soql = "SELECT Id, Name, Industry FROM Account WHERE Industry = 'Technology' LIMIT 10";
    let result = client.rest().query::<Account>(soql).await?;

    // Process results
    for account in result.records {
        println!("{}: {} ({})",
            account.id,
            account.name,
            account.industry.unwrap_or_default()
        );
    }

    Ok(())
}
