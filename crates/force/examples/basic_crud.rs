//! Basic CRUD Example
//!
//! This example demonstrates Create, Read, Update, and Delete operations.
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
//! cargo run --example basic_crud --features rest
//! ```

#[cfg(feature = "rest")]
mod example {
    use anyhow::Context;
    use force::auth::ClientCredentials;
    use force::client::builder;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Debug)]
    struct Account {
        #[serde(rename = "Name")]
        name: String,
        #[serde(rename = "Industry")]
        industry: String,
        #[serde(rename = "Description")]
        description: Option<String>,
    }

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

        // 1. Create
        println!("═══ Creating Account ═══");
        let create = Account {
            name: "Rust Test Account".to_string(),
            industry: "Technology".to_string(),
            description: Some("Created by force-rs example".to_string()),
        };

        let create_value = serde_json::to_value(&create)?;
        let created = client.rest().create("Account", &create_value).await?;
        let account_id = created
            .id
            .ok_or_else(|| anyhow::anyhow!("Created object has no ID"))?;
        println!("✓ Created Account with ID: {account_id}");

        // 2. Read
        println!("\n═══ Reading Account ═══");
        let fetched_value = client.rest().get("Account", &account_id).await?;
        let fetched: Account = serde_json::from_value(fetched_value)?;
        println!("✓ Fetched: {} ({})", fetched.name, fetched.industry);

        // 3. Update
        println!("\n═══ Updating Account ═══");
        let update = Account {
            description: Some("Updated description".to_string()),
            ..fetched
        };
        let update_value = serde_json::to_value(&update)?;
        client
            .rest()
            .update("Account", &account_id, &update_value)
            .await?;
        println!("✓ Update successful");

        let refreshed_value = client.rest().get("Account", &account_id).await?;
        let updated: Account = serde_json::from_value(refreshed_value)?;
        println!("  New Description: {:?}", updated.description);

        // 4. Delete
        println!("\n═══ Deleting Account ═══");
        client.rest().delete("Account", &account_id).await?;
        println!("✓ Account deleted");

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
