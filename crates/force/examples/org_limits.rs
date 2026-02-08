//! Organization Limits Example
//!
//! This example demonstrates how to retrieve and display Salesforce org limits,
//! including API usage, storage capacity, and other resource constraints.
//!
//! # Setup
//!
//! Set the following environment variables:
//! - `SF_CLIENT_ID` - OAuth client ID (Connected App Consumer Key)
//! - `SF_CLIENT_SECRET` - OAuth client secret (Connected App Consumer Secret)
//!
//! # Run
//!
//! ```bash
//! cargo run --example org_limits
//! ```

use force::auth::ClientCredentials;
use force::client::builder;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing for logging
    tracing_subscriber::fmt::init();

    // Read credentials from environment variables
    let client_id =
        std::env::var("SF_CLIENT_ID").expect("SF_CLIENT_ID environment variable not set");
    let client_secret =
        std::env::var("SF_CLIENT_SECRET").expect("SF_CLIENT_SECRET environment variable not set");

    println!("Authenticating with Salesforce...");

    // Create client with OAuth 2.0 Client Credentials flow
    let auth = ClientCredentials::new(client_id, client_secret);
    let client = builder().authenticate(auth).build().await?;

    println!("✓ Authentication successful\n");

    // Retrieve organization limits
    println!("Fetching organization limits...");
    let limits = client.rest().limits().await?;

    println!("✓ Retrieved limits\n");

    // Display API usage
    println!("═══ API USAGE ═══");
    let api = &limits.daily_api_requests;
    println!(
        "Daily API Requests: {}/{} ({:.1}% used)",
        api.used.unwrap_or(api.max - api.remaining),
        api.max,
        api.percentage_used()
    );

    if api.is_above_threshold(80.0) {
        println!("⚠️  WARNING: API usage above 80% threshold!");
    }

    if api.is_at_limit() {
        println!("❌ ERROR: Daily API limit reached!");
    }

    // Display storage limits
    println!("\n═══ STORAGE ═══");
    let data_storage = &limits.data_storage_mb;
    println!(
        "Data Storage: {}/{} MB ({:.1}% used)",
        data_storage.max - data_storage.remaining,
        data_storage.max,
        data_storage.percentage_used()
    );

    let file_storage = &limits.file_storage_mb;
    println!(
        "File Storage: {}/{} MB ({:.1}% used)",
        file_storage.max - file_storage.remaining,
        file_storage.max,
        file_storage.percentage_used()
    );

    // Display email limits
    println!("\n═══ EMAIL LIMITS ═══");
    let workflow_email = &limits.daily_workflow_emails;
    println!(
        "Daily Workflow Emails: {}/{} ({:.1}% used)",
        workflow_email
            .used
            .unwrap_or(workflow_email.max - workflow_email.remaining),
        workflow_email.max,
        workflow_email.percentage_used()
    );

    let mass_email = &limits.mass_email;
    println!(
        "Mass Emails: {}/{} remaining",
        mass_email.remaining, mass_email.max
    );

    let single_email = &limits.single_email;
    println!(
        "Single Emails: {}/{} remaining",
        single_email.remaining, single_email.max
    );

    // Display batch/async limits
    println!("\n═══ BATCH & ASYNC ═══");
    let batch_apex = &limits.daily_batch_apex_executions;
    println!(
        "Daily Batch Apex: {}/{} remaining",
        batch_apex.remaining, batch_apex.max
    );

    let async_apex = &limits.daily_async_apex_executions;
    println!(
        "Daily Async Apex: {}/{} remaining",
        async_apex.remaining, async_apex.max
    );

    // Display streaming limits
    println!("\n═══ STREAMING API ═══");
    let streaming = &limits.daily_streaming_api_events;
    println!(
        "Daily Streaming Events: {}/{} remaining",
        streaming.remaining, streaming.max
    );

    // Check for any additional limits
    if !limits.additional_limits.is_empty() {
        println!("\n═══ ADDITIONAL LIMITS ═══");
        for (name, limit) in &limits.additional_limits {
            println!(
                "{}: {}/{} ({:.1}% used)",
                name,
                limit.max - limit.remaining,
                limit.max,
                limit.percentage_used()
            );
        }
    }

    // Summary warnings
    println!("\n═══ SUMMARY ═══");
    let mut warnings = Vec::new();

    if api.is_above_threshold(90.0) {
        warnings.push("⚠️  API usage critical (>90%)");
    } else if api.is_above_threshold(75.0) {
        warnings.push("⚠️  API usage high (>75%)");
    }

    if data_storage.is_above_threshold(90.0) {
        warnings.push("⚠️  Data storage critical (>90%)");
    } else if data_storage.is_above_threshold(75.0) {
        warnings.push("⚠️  Data storage high (>75%)");
    }

    if file_storage.is_above_threshold(90.0) {
        warnings.push("⚠️  File storage critical (>90%)");
    } else if file_storage.is_above_threshold(75.0) {
        warnings.push("⚠️  File storage high (>75%)");
    }

    if warnings.is_empty() {
        println!("✓ All limits within normal ranges");
    } else {
        for warning in warnings {
            println!("{}", warning);
        }
    }

    Ok(())
}
