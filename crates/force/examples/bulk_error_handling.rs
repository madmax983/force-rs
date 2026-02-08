//! Bulk API Error Handling Example
//!
//! This example demonstrates comprehensive error handling patterns for bulk operations:
//! 1. Retrieving failed records from completed jobs
//! 2. Handling job failures (Failed/Aborted states)
//! 3. Network error retry logic with exponential backoff
//! 4. CSV validation errors
//! 5. Parsing failed/unprocessed results
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

use force::api::bulk::csv::deserialize_from_csv;
use force::api::bulk::ingest::IngestJobBuilder;
use force::api::bulk::types::JobOperation;
use force::auth::ClientCredentials;
use force::client::builder;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
struct Account {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Industry", skip_serializing_if = "Option::is_none")]
    industry: Option<String>,
}

#[derive(Deserialize, Debug)]
struct FailedRecord {
    #[serde(rename = "Name", default)]
    name: String,
    #[serde(rename = "sf__Error")]
    error: String,
    #[serde(rename = "sf__Id", default)]
    id: String,
}

#[derive(Deserialize, Debug)]
struct SuccessRecord {
    #[serde(rename = "sf__Id")]
    id: String,
    #[serde(rename = "sf__Created")]
    created: String,
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

    // Example 1: Handling validation errors and retrieving failed records
    println!("═══ Example 1: Processing Failed Records ═══");
    let mixed_accounts = vec![
        Account {
            name: "Valid Corp".to_string(),
            industry: Some("Technology".to_string()),
        },
        Account {
            name: "".to_string(), // Invalid - empty name (required field)
            industry: Some("Finance".to_string()),
        },
        Account {
            name: "Another Valid Corp".to_string(),
            industry: Some("Manufacturing".to_string()),
        },
    ];

    // Serialize to CSV
    let mut csv_buffer = Vec::new();
    force::api::bulk::csv::serialize_to_csv(&mixed_accounts, &mut csv_buffer)?;

    // Create and run the job using typestate pattern
    println!("Creating ingest job for Account...");
    let job = IngestJobBuilder::new("Account", JobOperation::Insert)
        .build(&client.bulk())
        .await?;
    println!("✓ Job created");

    println!("Uploading CSV data...");
    let job = job.upload(&csv_buffer).await?;
    println!("✓ CSV uploaded");

    println!("Closing job and starting processing...");
    let job = job.close().await?;
    println!("✓ Job closed");

    println!("Polling for completion (may take 10-30 seconds)...");
    let job = job.poll_until_complete().await?;
    println!("✓ Job completed: {}", job.job_id());

    // Retrieve and analyze results
    println!("\nRetrieving successful records...");
    let successful_csv = job.successful_results().await?;
    let successful_records: Vec<SuccessRecord> = deserialize_from_csv(&successful_csv[..])?;
    println!("✓ Successfully processed: {} records", successful_records.len());
    for record in &successful_records {
        println!("  - Created record: {}", record.id);
    }

    println!("\nRetrieving failed records...");
    let failed_csv = job.failed_results().await?;
    if !failed_csv.is_empty() {
        let failed_records: Vec<FailedRecord> = deserialize_from_csv(&failed_csv[..])?;
        println!("⚠ Failed: {} records", failed_records.len());
        for record in &failed_records {
            println!("  - Error: {} (Name: {})", record.error, record.name);
        }
    } else {
        println!("✓ No failed records");
    }

    println!("\nRetrieving unprocessed records...");
    let unprocessed_csv = job.unprocessed_results().await?;
    if !unprocessed_csv.is_empty() {
        println!("⚠ Unprocessed: {} bytes", unprocessed_csv.len());
    } else {
        println!("✓ No unprocessed records");
    }

    // Example 2: Handling CSV validation errors
    println!("\n═══ Example 2: CSV Validation Errors ═══");
    let invalid_csv = b"InvalidHeader,AnotherHeader\nValue1,Value2\n";

    let job = IngestJobBuilder::new("Account", JobOperation::Insert)
        .build(&client.bulk())
        .await?;

    match job.upload(invalid_csv).await {
        Ok(job) => {
            println!("Upload succeeded (may fail during processing)");
            let job = job.close().await?;
            match job.poll_until_complete().await {
                Ok(job) => {
                    println!("✓ Job completed (check for failed records)");
                    let failed_csv = job.failed_results().await?;
                    if !failed_csv.is_empty() {
                        println!("⚠ CSV validation failed - check Salesforce for details");
                    }
                }
                Err(e) => {
                    println!("✓ CSV validation error caught: {}", e);
                    println!("Tip: Ensure CSV headers match Salesforce field names");
                }
            }
        }
        Err(e) => {
            println!("✓ Upload error caught: {}", e);
            println!("Tip: Check CSV format and field names");
        }
    }

    // Example 3: Retry logic with exponential backoff
    println!("\n═══ Example 3: Retry Logic with Backoff ═══");
    let retry_accounts = vec![Account {
        name: "Retry Test Corp".to_string(),
        industry: Some("Technology".to_string()),
    }];

    let mut retry_csv = Vec::new();
    force::api::bulk::csv::serialize_to_csv(&retry_accounts, &mut retry_csv)?;

    const MAX_RETRIES: u32 = 3;
    let mut attempt = 0;
    let mut backoff_secs = 1;

    loop {
        attempt += 1;
        println!("Attempt {} of {}", attempt, MAX_RETRIES);

        let result = async {
            let job = IngestJobBuilder::new("Account", JobOperation::Insert)
                .build(&client.bulk())
                .await?;
            let job = job.upload(&retry_csv).await?;
            let job = job.close().await?;
            job.poll_until_complete().await
        }
        .await;

        match result {
            Ok(job) => {
                println!("✓ Success on attempt {}", attempt);
                println!("Job ID: {}", job.job_id());
                break;
            }
            Err(e) => {
                if attempt >= MAX_RETRIES {
                    eprintln!("✗ Failed after {} attempts: {}", MAX_RETRIES, e);
                    println!("Tip: Check network connectivity and Salesforce status");
                    break;
                }

                eprintln!("⚠ Attempt {} failed: {}", attempt, e);
                println!("Retrying in {} seconds...", backoff_secs);
                tokio::time::sleep(tokio::time::Duration::from_secs(backoff_secs)).await;
                backoff_secs = std::cmp::min(backoff_secs * 2, 30); // Cap at 30s
            }
        }
    }

    // Example 4: Aborting a job before completion
    println!("\n═══ Example 4: Job Abort Handling ═══");
    let job = IngestJobBuilder::new("Account", JobOperation::Insert)
        .build(&client.bulk())
        .await?;
    println!("Created job in Open state");

    // Abort before uploading data
    match job.abort().await {
        Ok(()) => {
            println!("✓ Job aborted successfully");
            println!("Tip: Use abort() to cancel jobs that are no longer needed");
        }
        Err(e) => {
            println!("⚠ Abort failed: {}", e);
        }
    }

    // Example 5: Handling query errors
    println!("\n═══ Example 5: Bulk Query Error Handling ═══");
    let invalid_soql = "SELECT InvalidField__c FROM Account WHERE InvalidCondition";

    match client.bulk().bulk_query::<Account>(invalid_soql).await {
        Ok(mut stream) => {
            println!("Query job created (checking for errors during processing)...");
            loop {
                match stream.next().await {
                    Ok(Some(record)) => {
                        println!("✓ Received record: {:?}", record);
                    }
                    Ok(None) => {
                        println!("✓ Stream exhausted");
                        break;
                    }
                    Err(e) => {
                        println!("✓ Query error caught during streaming: {}", e);
                        println!("Tip: Validate SOQL syntax before submitting");
                        break;
                    }
                }
            }
        }
        Err(e) => {
            println!("✓ Query error caught at job creation: {}", e);
            println!("Error details: {}", e);
            println!("Tip: Check SOQL syntax, field names, and object access permissions");
        }
    }

    println!("\n═══ Error Handling Examples Complete ═══");
    println!("\nKey Takeaways:");
    println!("1. Always retrieve failed_results() after job completion");
    println!("2. Use exponential backoff for network errors");
    println!("3. Validate CSV headers match Salesforce field names");
    println!("4. Check job state (Failed/Aborted) during polling");
    println!("5. Parse CSV results to get detailed error messages");

    Ok(())
}
