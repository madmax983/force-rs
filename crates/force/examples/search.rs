//! Search (SOSL) Example

#[cfg(feature = "rest")]
mod example {
    use anyhow::Context;
    use force::auth::ClientCredentials;
    use force::client::builder;
    use force::types::DynamicSObject;

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

        let query =
            "FIND {test} IN ALL FIELDS RETURNING Account(Name), Contact(FirstName, LastName)";
        println!("Searching: {}", query);

        let results = client.rest().search(query).await?;

        println!("Found {} results:", results.search_records.len());
        for record_set in results.search_records {
            let obj_type = &record_set.attributes.type_;

            for record in record_set.records {
                if obj_type == "Account" {
                    let name = record.get("Name").and_then(|v| v.as_str()).unwrap_or("N/A");
                    println!("- Account: {}", name);
                } else if obj_type == "Contact" {
                    let first = record
                        .get("FirstName")
                        .and_then(|v| v.as_str())
                        .unwrap_or("N/A");
                    let last = record
                        .get("LastName")
                        .and_then(|v| v.as_str())
                        .unwrap_or("N/A");
                    println!("- Contact: {} {}", first, last);
                } else {
                    println!("- Unknown type: {}", obj_type);
                }
            }
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
