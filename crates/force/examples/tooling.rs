//! Tooling API Example
//!
//! Demonstrates using the Salesforce Tooling API for Apex class management,
//! anonymous Apex execution, code completions, and test execution.
//!
//! The Tooling API mirrors the REST API's CRUD, Query, and Describe operations
//! but routes requests under the `/tooling/` prefix. In addition it exposes
//! development-specific endpoints: executeAnonymous, runTests, and completions.
//!
//! # Setup
//!
//! Set the following environment variables:
//! - `SF_CLIENT_ID`     - OAuth client ID (Connected App Consumer Key)
//! - `SF_CLIENT_SECRET`  - OAuth client secret (Connected App Consumer Secret)
//!
//! # Run
//!
//! ```bash
//! SF_CLIENT_ID=xxx SF_CLIENT_SECRET=yyy cargo run --example tooling --features tooling
//! ```

#[cfg(feature = "tooling")]
mod example {
    use anyhow::Context;
    use force::api::rest_operation::RestOperation;
    use force::api::tooling::{CompletionsType, RunTestsRequest, TestItem};
    use force::auth::ClientCredentials;
    use force::client::ForceClientBuilder;
    use force::types::QueryResult;

    fn required_env(name: &str) -> anyhow::Result<String> {
        std::env::var(name).with_context(|| format!("{name} environment variable not set"))
    }

    pub async fn main() -> anyhow::Result<()> {
        tracing_subscriber::fmt::init();

        let client_id = required_env("SF_CLIENT_ID")?;
        let client_secret = required_env("SF_CLIENT_SECRET")?;

        println!("Authenticating with Salesforce...");
        let auth = ClientCredentials::new_production(client_id, client_secret);
        let client = ForceClientBuilder::new().authenticate(auth).build().await?;
        let tooling = client.tooling();
        println!("Authentication successful\n");

        // ── 1. Query Apex Classes ───────────────────────────────────────
        println!("=== Querying Apex Classes ===");
        let result: QueryResult<serde_json::Value> = tooling
            .query("SELECT Id, Name, Status FROM ApexClass LIMIT 5")
            .await?;
        println!("Found {} Apex classes", result.total_size);
        for record in &result.records {
            println!(
                "  {} - {}",
                record["Name"].as_str().unwrap_or("unknown"),
                record["Status"].as_str().unwrap_or("unknown")
            );
        }

        // ── 2. Execute Anonymous Apex ───────────────────────────────────
        println!("\n=== Execute Anonymous Apex ===");
        let exec_result = tooling
            .execute_anonymous("System.debug('Hello from force-rs!');")
            .await?;
        if exec_result.is_success() {
            println!("  Execution successful!");
        } else if exec_result.is_compile_error() {
            println!(
                "  Compile error: {}",
                exec_result.compile_problem.unwrap_or_default()
            );
        } else if exec_result.is_runtime_error() {
            println!(
                "  Runtime error: {}",
                exec_result.exception_message.unwrap_or_default()
            );
        }

        // ── 3. Describe a Tooling Object ────────────────────────────────
        println!("\n=== Describe ApexClass ===");
        let describe = tooling.describe("ApexClass").await?;
        println!("  {} has {} fields", describe.name, describe.fields.len());
        for field in describe.fields.iter().take(5) {
            println!("    {} ({:?})", field.name, field.type_);
        }

        // ── 4. Run Tests (synchronous) ──────────────────────────────────
        // Uncomment when you have test classes available in your org:
        //
        // println!("\n=== Run Apex Tests ===");
        // let request = RunTestsRequest {
        //     tests: vec![TestItem {
        //         class_id: "01pXX0000000001AAA".to_string(),
        //         test_methods: None,
        //     }],
        //     max_failed_tests: Some(-1),
        // };
        // let test_result = tooling.run_tests(&request).await?;
        // println!(
        //     "  Ran {} tests: {} passed, {} failed ({:.0}ms)",
        //     test_result.num_tests_run,
        //     test_result.successes.len(),
        //     test_result.num_failures,
        //     test_result.total_time,
        // );

        // Keep RunTestsRequest and TestItem available for the compiler
        let _ = (RunTestsRequest {
            tests: vec![TestItem {
                class_id: String::new(),
                test_methods: None,
            }],
            max_failed_tests: None,
        },);

        // ── 5. Code Completions ─────────────────────────────────────────
        println!("\n=== Code Completions ===");
        let completions = tooling
            .completions(CompletionsType::Apex, "System.d")
            .await?;
        println!("  Got {} completion entries", completions.completions.len());

        println!("\nDone!");
        Ok(())
    }
}

#[cfg(feature = "tooling")]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    example::main().await
}

#[cfg(not(feature = "tooling"))]
fn main() {
    println!("This example requires the 'tooling' feature.");
    println!("Run with: cargo run --example tooling --features tooling");
}
