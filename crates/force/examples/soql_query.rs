#![cfg(feature = "rest")]
#![cfg(feature = "rest")]
//! SOQL Query with Typed Results Example

use anyhow::Context;
use force::auth::ClientCredentials;
use force::client::{ForceClient, builder};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct Account {
    #[serde(rename = "Id")]
    id: String,
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Industry")]
    industry: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Contact {
    #[serde(rename = "Id")]
    id: String,
    #[serde(rename = "LastName")]
    last_name: String,
    #[serde(rename = "Email")]
    email: Option<String>,
}

#[derive(Debug, Deserialize)]
struct IndustryStats {
    #[serde(rename = "Industry")]
    industry: Option<String>,
    #[serde(rename = "TotalAccounts")]
    total_accounts: i32,
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
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let client = build_client().await?;

    let accounts = client
        .query::<Account>("SELECT Id, Name, Industry FROM Account ORDER BY Name LIMIT 10")
        .await?;
    println!("Accounts fetched: {}", accounts.records.len());
    for account in &accounts.records {
        let industry = account.industry.as_deref().unwrap_or("(none)");
        println!("- {} ({}) [{industry}]", account.name, account.id);
    }

    let mut contacts = client
        .query::<Contact>("SELECT Id, LastName, Email FROM Contact ORDER BY LastName LIMIT 5")
        .await?;
    let mut pages = 1;
    let mut count = contacts.records.len();
    while let Some(next) = contacts.next_records_url.clone() {
        contacts = client.query_more::<Contact>(&next).await?;
        pages += 1;
        count += contacts.records.len();
    }
    println!("\nContacts fetched across {pages} page(s): {count}");

    let stats = client
        .query::<IndustryStats>(
            "SELECT Industry, COUNT(Id) TotalAccounts FROM Account WHERE Industry != null GROUP BY Industry LIMIT 5",
        )
        .await?;
    println!("\nIndustry stats:");
    for row in &stats.records {
        println!(
            "- {}: {}",
            row.industry.as_deref().unwrap_or("(none)"),
            row.total_accounts
        );
    }

    Ok(())
}
