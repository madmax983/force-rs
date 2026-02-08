//! Bulk API Error Handling Example
//!
//! This example demonstrates error handling patterns for bulk operations.
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
//! cargo run --example bulk_error_handling --features bulk
//! ```

use force::auth::ClientCredentials;
use force::client::builder;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct Account {
    #[serde(rename = "Name")]
    name: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Get credentials from environment
    let client_id =
        std::env::var("SF_CLIENT_ID").expect("SF_CLIENT_ID environment variable not set");
    let client_secret =
        std::env::var("SF_CLIENT_SECRET").expect("SF_CLIENT_SECRET environment variable not set");

    println!("═══ Authenticating ═══");
    let auth = ClientCredentials::new(
        client_id,
        client_secret,
        "https://login.salesforce.com/services/oauth2/token",
    );
    let client = builder().authenticate(auth).build().await?;
    println!("✓ Authentication successful\n");

    // Example 1: Handling validation errors (empty required field)
    println!("═══ Example 1: Validation Errors ═══");
    let invalid_accounts = vec![
        Account {
            name: "Valid Account".to_string(),
        },
        Account {
            name: "".to_string(), // Invalid - empty name
        },
    ];

    match client
        .bulk()
        .bulk_insert("Account", &invalid_accounts)
        .await
    {
        Ok(job_info) => {
            println!("Job completed: {}", job_info.id);
            println!(
                "Processed: {}, Failed: {}",
                job_info.number_records_processed.unwrap_or(0),
                job_info.number_records_failed.unwrap_or(0)
            );

            if job_info.number_records_failed.unwrap_or(0) > 0 {
                println!("⚠ Some records failed validation");
                println!("Check failed records in Salesforce job results");
            }
        }
        Err(e) => {
            eprintln!("✗ Bulk insert failed: {}", e);
            return Err(e.into());
        }
    }

    // Example 2: Handling network errors with retry logic
    println!("\n═══ Example 2: Retry Logic ═══");
    let accounts = vec![Account {
        name: "Test Account".to_string(),
    }];

    const MAX_RETRIES: u32 = 3;
    let mut attempt = 0;

    loop {
        attempt += 1;
        println!("Attempt {} of {}", attempt, MAX_RETRIES);

        match client.bulk().bulk_insert("Account", &accounts).await {
            Ok(job_info) => {
                println!("✓ Success on attempt {}", attempt);
                println!("Job ID: {}", job_info.id);
                break;
            }
            Err(e) => {
                if attempt >= MAX_RETRIES {
                    eprintln!("✗ Failed after {} attempts: {}", MAX_RETRIES, e);
                    return Err(e.into());
                }

                eprintln!("⚠ Attempt {} failed: {}", attempt, e);
                println!("Retrying in 2 seconds...");
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            }
        }
    }

    // Example 3: Handling query errors
    println!("\n═══ Example 3: Query Errors ═══");
    let invalid_soql = "SELECT InvalidField FROM Account";

    match client.bulk().bulk_query::<Account>(invalid_soql).await {
        Ok(_stream) => {
            println!("Query succeeded (unexpected)");
        }
        Err(e) => {
            println!("✓ Query error caught correctly");
            println!("Error: {}", e);
            println!("Tip: Check SOQL syntax and field names");
        }
    }

    println!("\n═══ Error Handling Complete ═══");
    Ok(())
}
