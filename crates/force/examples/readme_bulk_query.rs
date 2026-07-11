// Requires the "bulk" feature: force = { version = "0.3", features = ["bulk"] }
#![allow(missing_docs)]

#[cfg(feature = "bulk")]
mod example {
    use force::auth::ClientCredentials;
    use force::client::ForceClientBuilder;
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    struct Contact {
        #[serde(rename = "Id")]
        id: String,
        #[serde(rename = "Email")]
        email: Option<String>,
    }

    pub async fn main() -> anyhow::Result<()> {
        let auth = ClientCredentials::new_my_domain(
            "client-id",
            "client-secret",
            "https://your-org.my.salesforce.com",
        );
        let client = ForceClientBuilder::new().authenticate(auth).build().await?;

        // Create bulk query job and stream results
        let mut stream = client
            .bulk()
            .query::<Contact>("SELECT Id, Email FROM Contact WHERE Email != null")
            .await?;

        let mut count = 0;
        while let Some(contact) = stream.next().await? {
            println!(
                "Processing: {} ({})",
                contact.id,
                contact.email.unwrap_or_default()
            );
            count += 1;
        }

        println!("Streamed {count} contacts");
        Ok(())
    }
}

#[cfg(feature = "bulk")]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    example::main().await
}

#[cfg(not(feature = "bulk"))]
fn main() {
    println!("This example requires the 'bulk' feature.");
}
