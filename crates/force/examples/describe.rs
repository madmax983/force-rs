//! Describe API Example
//!
//! Demonstrates global describe and object-specific describe operations.

#[cfg(feature = "rest")]
mod example {
    use anyhow::Context;
    use force::auth::ClientCredentials;
    use force::client::{ForceClient, builder};

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

    pub async fn main() -> anyhow::Result<()> {
        tracing_subscriber::fmt::init();
        let client = build_client().await?;

        let global = client.rest().describe_global().await?;
        println!("Total sObjects: {}", global.sobjects.len());

        for sobject in global.sobjects.iter().take(10) {
            println!("- {} ({})", sobject.label, sobject.name);
        }

        let account = client.rest().describe("Account").await?;
        println!("\nAccount fields: {}", account.fields.len());

        for field in account.fields.iter().take(15) {
            println!("- {} ({:?})", field.name, field.type_);
        }

        Ok(())
    }
}

#[cfg(feature = "rest")]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    example::main().await
}

#[cfg(not(feature = "rest"))]
fn main() {
    println!("This example requires the 'rest' feature.");
}
