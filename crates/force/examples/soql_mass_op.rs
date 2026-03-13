//! Example: Mass Update Records using SOQL and Composite API
//!
//! This example demonstrates how to find specific records using a SOQL query
//! and instantly update all of them using the experimental `SoqlMassOp` utility.
//!
//! Run with: `cargo run --example soql_mass_op --features full`

use force::api::SoqlQueryBuilder;
use force::auth::ClientCredentials;
use force::client::ForceClientBuilder;
use force::experimental::SoqlMassOp;
use serde_json::json;
use std::env;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let client_id = env::var("SF_CLIENT_ID").unwrap_or_else(|_| "dummy_client_id".to_string());
    let client_secret =
        env::var("SF_CLIENT_SECRET").unwrap_or_else(|_| "dummy_client_secret".to_string());

    // Authenticate
    let auth = ClientCredentials::new_production(client_id, client_secret);
    let client = ForceClientBuilder::new().authenticate(auth).build().await?;

    println!("Finding Contact records that are marked as 'New'...");

    // 1. Build a safe SOQL query to find the target records.
    // The `Id` field MUST be selected for mass operations.
    let query = SoqlQueryBuilder::new()
        .select(&["Id"])
        .from("Contact")
        .where_eq("Status", "New")
        .limit(100); // Limit added for safety during the example

    // 2. Define the exact fields and values you want to mass update.
    let updates = json!({
        "Status": "Processed",
        "Description": "Updated automatically by SoqlMassOp via force-rs"
    });

    println!("Executing mass update using Composite Batch...");

    // 3. Execute the mass update!
    // This will query the records and send Composite Batch requests
    // in chunks of 25 to perform the updates.
    let mass_op = SoqlMassOp::new(&client, query).halt_on_error(false);

    let stats = mass_op.update_all(updates).await?;

    println!("--- Mass Update Complete ---");
    println!("Records Found & Processed : {}", stats.records_processed);
    println!("Successful Updates        : {}", stats.ops_succeeded);
    println!("Failed Updates            : {}", stats.ops_failed);

    Ok(())
}
