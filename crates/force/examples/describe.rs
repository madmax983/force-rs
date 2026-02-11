//! `SObject` Describe Example
//!
//! This example demonstrates how to retrieve metadata for Salesforce objects.
//!
//! # Setup
//!
//! Set the following environment variables:
//! - `SF_CLIENT_ID` - OAuth client ID
//! - `SF_CLIENT_SECRET` - OAuth client secret
//!
//! # Run
//!
//! ```bash
//! cargo run --example describe --features rest
//! ```

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
        // Initialize tracing
        tracing_subscriber::fmt::init();

        // Get credentials from environment
        let client_id = required_env("SF_CLIENT_ID")?;
        let client_secret = required_env("SF_CLIENT_SECRET")?;

        println!("═══ Authenticating ═══");
        let auth = ClientCredentials::new(
            client_id,
            client_secret,
            "https://login.salesforce.com/services/oauth2/token",
        );
        let client = builder().authenticate(auth).build().await?;
        println!("✓ Authentication successful\n");

        // Global Describe (list all objects)
        println!("═══ Global Describe ═══");
        let global = client.rest().describe_global().await?;
        println!("Found {} objects in the org.", global.sobjects.len());

        // Find Account object metadata
        if let Some(account_meta) = global.sobjects.iter().find(|o| o.name == "Account") {
            println!("\nAccount Object Summary:");
            println!("  Label: {}", account_meta.label);
            println!(
                "  Key Prefix: {}",
                account_meta.key_prefix.as_deref().unwrap_or("None")
            );
            println!("  Custom: {}", account_meta.custom);
        }

        // Full Describe for Account (fields, relationships, etc.)
        println!("\n═══ Account SObject Describe ═══");
        let account = client.rest().describe("Account").await?;

        println!("Fields: {}", account.fields.len());
        println!("Child Relationships: {}", account.child_relationships.len());

        println!("\nTop 5 Fields:");
        for field in account.fields.iter().take(5) {
            println!("  - {} ({:?})", field.name, field.type_);
        }

        Ok(())
    }
}

#[cfg(feature = "rest")]
fn main() -> anyhow::Result<()> {
    example::main()
}

#[cfg(not(feature = "rest"))]
fn main() {
    println!("This example requires the 'rest' feature.");
}
