//! Query Plan API example (requires the "rest" feature).
//!
//! This example demonstrates how to use the Query Plan API (`explain`)
//! to analyze the performance cost of SOQL queries.
//!
//! Run with:
//! cargo run --example `query_plan`

use force::auth::ClientCredentials;
use force::client::ForceClientBuilder;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let auth = ClientCredentials::new_my_domain(
        std::env::var("SF_CLIENT_ID").unwrap_or_else(|_| "client-id".to_string()),
        std::env::var("SF_CLIENT_SECRET").unwrap_or_else(|_| "client-secret".to_string()),
        std::env::var("SF_MY_DOMAIN_URL")
            .unwrap_or_else(|_| "https://your-org.my.salesforce.com".to_string()),
    );

    let client = ForceClientBuilder::new().authenticate(auth).build().await?;

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
