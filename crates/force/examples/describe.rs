//! Describe SObject Example

#[cfg(feature = "rest")]
mod example {
    use anyhow::Context;
    use force::auth::ClientCredentials;
    use force::client::builder;

    fn required_env(name: &str) -> anyhow::Result<String> {
        std::env::var(name).with_context(|| format!("{name} environment variable not set"))
    }

    #[tokio::main]
    pub async fn main() -> anyhow::Result<()> {
        tracing_subscriber::fmt::init();

        let client_id = required_env("SF_CLIENT_ID")?;
        let client_secret = required_env("SF_CLIENT_SECRET")?;

        let auth = ClientCredentials::new(
            client_id,
            client_secret,
            "https://login.salesforce.com/services/oauth2/token",
        );
        let client = builder().authenticate(auth).build().await?;

        // Basic description
        println!("Describing Account object...");
        let desc = client.rest().describe("Account").await?;

        println!("Label: {}", desc.label);
        println!("Queryable: {}", desc.queryable);
        println!("Createable: {}", desc.createable);
        println!("Fields: {}", desc.fields.len());

        if let Some(field) = desc.fields.iter().find(|f| f.name == "Name") {
            println!("\nField 'Name':");
            println!("  Type: {}", field.type_);
            println!("  Label: {}", field.label);
            println!("  Nillable: {}", field.nillable);
        }

        Ok(())
    }
}

fn main() -> anyhow::Result<()> {
    #[cfg(feature = "rest")]
    {
        example::main()
    }
    #[cfg(not(feature = "rest"))]
    {
        println!("This example requires the 'rest' feature.");
        Ok(())
    }
}
