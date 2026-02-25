// Requires the "bulk" feature: force = { version = "0.1", features = ["bulk"] }
use force::client::ForceClientBuilder;
use force::auth::ClientCredentials;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Contact {
    #[serde(rename = "Id")]
    id: String,
    #[serde(rename = "Email")]
    email: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let auth = ClientCredentials::new_production(
        "client-id",
        "client-secret",
    );
    let client = ForceClientBuilder::new().authenticate(auth).build().await?;

    // Create bulk query job and stream results
    let mut stream = client.bulk()
        .query::<Contact>(
            "SELECT Id, Email FROM Contact WHERE Email != null"
        )
        .await?;

    let mut count = 0;
    while let Some(contact) = stream.next().await? {
        println!("Processing: {} ({})", contact.id, contact.email.unwrap_or_default());
        count += 1;
    }

    println!("Streamed {} contacts", count);
    Ok(())
}
