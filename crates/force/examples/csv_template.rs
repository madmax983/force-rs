//! CSV Template Generator Example
//!
//! Demonstrates how to generate a CSV header template for bulk data loading.

#[cfg(feature = "schema")]
mod example {
    use force::client::ForceClientBuilder;
    use force::schema::generate_csv_template;
    use force::auth::ClientCredentials;
    use force::api::RestOperation;
    use anyhow::Context;

    fn required_env(name: &str) -> anyhow::Result<String> {
        std::env::var(name).with_context(|| format!("{} environment variable not set", name))
    }

    pub async fn main() -> anyhow::Result<()> {
        let client_id = required_env("SF_CLIENT_ID")?;
        let client_secret = required_env("SF_CLIENT_SECRET")?;
        let my_domain_url = required_env("SF_MY_DOMAIN_URL")?;

        let auth = ClientCredentials::new_my_domain(client_id, client_secret, my_domain_url);
        let client = ForceClientBuilder::new().authenticate(auth).build().await?;

        let describe = client.rest().describe("Account").await?;
        let csv_template = generate_csv_template(&describe);

        println!("Generated CSV Template for Account:");
        print!("{}", csv_template);

        Ok(())
    }
}

#[cfg(feature = "schema")]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    example::main().await
}

#[cfg(not(feature = "schema"))]
fn main() {
    println!("This example requires the 'schema' feature.");
}
