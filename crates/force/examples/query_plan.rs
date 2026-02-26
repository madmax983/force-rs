//! Query Plan API example (requires "nova" feature).
//!
//! This example demonstrates how to use the Query Plan API (`explain`)
//! to analyze the performance cost of SOQL queries.
//!
//! Run with:
//! cargo run --example `query_plan` --features nova

use force::client::ForceClientBuilder;
use force::auth::ClientCredentials;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Authenticate with OAuth 2.0 client credentials
    let auth = ClientCredentials::new_production(
        std::env::var("SF_CLIENT_ID").unwrap_or_else(|_| "client-id".to_string()),
        std::env::var("SF_CLIENT_SECRET").unwrap_or_else(|_| "client-secret".to_string()),
    );

    let client = ForceClientBuilder::new()
        .authenticate(auth)
        .build()
        .await?;

    let soql = "SELECT Id FROM Account WHERE Name LIKE 'A%'";
    println!("Analyzing query: {soql}");

    let explanation = client.rest().explain(soql).await?;

    for plan in explanation.plans {
        println!(
            "Plan: {}, Cost: {}",
            plan.leading_operation_type, plan.relative_cost
        );
        println!("  Cardinality: {}", plan.cardinality);
        println!("  SObject Type: {}", plan.sobject_type);

        if !plan.notes.is_empty() {
            println!("  Notes:");
            for note in plan.notes {
                println!("    - {}", note.description);
            }
        }
    }

    Ok(())
}
