// Requires the "bulk" feature: force = { version = "0.1", features = ["bulk"] }
use force::client::ForceClientBuilder;
use force::auth::ClientCredentials;
use serde::Serialize;

#[derive(Serialize)]
struct Account {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Industry")]
    industry: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let auth = ClientCredentials::new_production(
        "client-id",
        "client-secret",
    );
    let client = ForceClientBuilder::new().authenticate(auth).build().await?;

    let accounts = vec![
        Account { name: "Acme Corp".into(), industry: "Technology".into() },
        Account { name: "Global Ltd".into(), industry: "Manufacturing".into() },
    ];

    // Convenience method handles: create job → upload CSV → close → poll
    let job_info = client.bulk().insert("Account", &accounts).await?;

    println!("Processed: {}, Failed: {}",
        job_info.number_records_processed.unwrap_or(0),
        job_info.number_records_failed.unwrap_or(0)
    );

    Ok(())
}
