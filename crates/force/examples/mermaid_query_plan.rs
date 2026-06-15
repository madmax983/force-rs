//! Preview schema utility: Mermaid Query Plan Visualizer
//! Requires the `schema` feature: `force = { version = "0.1", features = ["schema"] }`

use force::auth::ClientCredentials;
use force::client::ForceClientBuilder;
use std::env;

#[cfg(feature = "schema")]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    use force::schema::generate_mermaid_query_plan;

    let client_id =
        env::var("SALESFORCE_CLIENT_ID").unwrap_or_else(|_| "your-client-id".to_string());
    let client_secret =
        env::var("SALESFORCE_CLIENT_SECRET").unwrap_or_else(|_| "your-client-secret".to_string());
    let my_domain_url = env::var("SALESFORCE_MY_DOMAIN_URL")
        .unwrap_or_else(|_| "https://your-org.my.salesforce.com".to_string());

    let soql = env::args()
        .nth(1)
        .unwrap_or_else(|| "SELECT Id FROM Account WHERE Name LIKE 'Acme%'".to_string());

    println!("Authenticating with Salesforce...");

    let auth = ClientCredentials::new_my_domain(&client_id, &client_secret, &my_domain_url);
    let client = ForceClientBuilder::new().authenticate(auth).build().await?;

    println!("Analyzing SOQL: {soql}");
    let response = client.rest().explain(&soql).await?;

    let mermaid_diagram = generate_mermaid_query_plan(&response);

    println!("\nGenerated Mermaid.js Diagram:\n");
    println!("```mermaid\n{mermaid_diagram}\n```\n");

    Ok(())
}

#[cfg(not(feature = "schema"))]
fn main() {
    println!(
        "This example requires the 'schema' feature. Run with: cargo run --example mermaid_query_plan --features schema"
    );
}
