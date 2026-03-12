use force::client::ForceClientBuilder;
use force::auth::ClientCredentials;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // For Sandbox, use: ClientCredentials::new_sandbox("client-id", "client-secret")
    let auth = ClientCredentials::new_production(
        "client-id",
        "client-secret",
    );
    let client = ForceClientBuilder::new().authenticate(auth).build().await?;

    let soql = "SELECT Id FROM Account WHERE Name LIKE 'A%'";
    let explanation = client.rest().explain(soql).await?;

    for plan in explanation.plans {
        println!("Plan: {}, Cost: {}", plan.leading_operation_type, plan.relative_cost);
        for note in plan.notes {
            println!("  Note: {}", note.description);
        }
    }
    Ok(())
}
