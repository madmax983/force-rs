//! SOQL Query with Typed Results Example
//!
//! This example demonstrates how to execute SOQL queries with typed deserialization,
//! handle pagination, and work with query results.
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
//! cargo run --example soql_query
//! ```

use force::auth::ClientCredentials;
use force::client::builder;
use serde::{Deserialize, Serialize};

// Define typed structs for query results
#[derive(Debug, Serialize, Deserialize)]
struct Account {
    #[serde(rename = "Id")]
    id: String,

    #[serde(rename = "Name")]
    name: String,

    #[serde(rename = "Industry")]
    industry: Option<String>,

    #[serde(rename = "Website")]
    website: Option<String>,

    #[serde(rename = "NumberOfEmployees")]
    number_of_employees: Option<i32>,

    #[serde(rename = "AnnualRevenue")]
    annual_revenue: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Contact {
    #[serde(rename = "Id")]
    id: String,

    #[serde(rename = "FirstName")]
    first_name: Option<String>,

    #[serde(rename = "LastName")]
    last_name: String,

    #[serde(rename = "Email")]
    email: Option<String>,

    #[serde(rename = "Phone")]
    phone: Option<String>,

    #[serde(rename = "Title")]
    title: Option<String>,

    #[serde(rename = "Account", skip_serializing_if = "Option::is_none")]
    account: Option<AccountSummary>,
}

#[derive(Debug, Serialize, Deserialize)]
struct AccountSummary {
    #[serde(rename = "Name")]
    name: String,

    #[serde(rename = "Industry")]
    industry: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Get credentials
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

    // EXAMPLE 1: Simple typed query
    println!("═══ EXAMPLE 1: Simple Typed Query ═══");
    let soql = "SELECT Id, Name, Industry, Website, NumberOfEmployees, AnnualRevenue \
                FROM Account \
                WHERE Industry = 'Technology' \
                ORDER BY Name \
                LIMIT 10";

    let result = client.query::<Account>(soql).await?;

    println!("Found {} total accounts", result.total_size);
    println!("Retrieved {} accounts in this page\n", result.len());

    for (idx, account) in result.records.iter().enumerate() {
        println!("{}. {} (ID: {})", idx + 1, account.name, account.id);
        if let Some(industry) = &account.industry {
            println!("   Industry: {}", industry);
        }
        if let Some(employees) = account.number_of_employees {
            println!("   Employees: {}", employees);
        }
        if let Some(revenue) = account.annual_revenue {
            println!("   Revenue: ${:.2}", revenue);
        }
        println!();
    }

    // EXAMPLE 2: Query with pagination
    println!("═══ EXAMPLE 2: Query with Pagination ═══");
    let soql_paginated = "SELECT Id, FirstName, LastName, Email, Phone, Title \
                          FROM Contact \
                          ORDER BY LastName \
                          LIMIT 5";

    let mut page_num = 1;
    let mut total_processed = 0;
    let mut current_result = client.query::<Contact>(soql_paginated).await?;

    println!(
        "Total contacts to retrieve: {}\n",
        current_result.total_size
    );

    loop {
        println!("--- Page {} ---", page_num);
        println!("Records in this page: {}", current_result.len());

        for contact in &current_result.records {
            let full_name = format!(
                "{} {}",
                contact.first_name.as_deref().unwrap_or(""),
                &contact.last_name
            )
            .trim()
            .to_string();

            println!("  • {} ({})", full_name, contact.id);
            if let Some(email) = &contact.email {
                println!("    Email: {}", email);
            }
            if let Some(title) = &contact.title {
                println!("    Title: {}", title);
            }
        }

        total_processed += current_result.len();
        println!(
            "Processed: {}/{}\n",
            total_processed, current_result.total_size
        );

        // Check if there are more pages
        if !current_result.has_more() {
            break;
        }

        // Fetch next page using the nextRecordsUrl
        if let Some(next_url) = &current_result.next_records_url {
            current_result = client.query_more(next_url).await?;
        } else {
            break;
        }

        page_num += 1;
    }

    println!(
        "✓ Retrieved all {} contacts across {} pages\n",
        total_processed, page_num
    );

    // EXAMPLE 3: Query with relationship (subquery)
    println!("═══ EXAMPLE 3: Query with Relationship ═══");
    let soql_relationship = "SELECT Id, FirstName, LastName, Email, Title, \
                             Account.Name, Account.Industry \
                             FROM Contact \
                             WHERE Account.Industry = 'Technology' \
                             LIMIT 5";

    let contacts_result = client
        .query::<Contact>(soql_relationship)
        .await?;

    println!("Contacts from Technology companies:\n");
    for contact in &contacts_result.records {
        let full_name = format!(
            "{} {}",
            contact.first_name.as_deref().unwrap_or(""),
            &contact.last_name
        )
        .trim()
        .to_string();

        println!("  • {}", full_name);
        if let Some(title) = &contact.title {
            println!("    Title: {}", title);
        }
        if let Some(account) = &contact.account {
            println!("    Company: {}", account.name);
            if let Some(industry) = &account.industry {
                println!("    Industry: {}", industry);
            }
        }
        println!();
    }

    // EXAMPLE 4: Aggregate query
    println!("═══ EXAMPLE 4: Aggregate Query ═══");

    #[derive(Debug, Deserialize)]
    struct IndustryStats {
        #[serde(rename = "Industry")]
        industry: Option<String>,

        #[serde(rename = "TotalAccounts")]
        total_accounts: i32,

        #[serde(rename = "AvgRevenue")]
        avg_revenue: Option<f64>,
    }

    let soql_aggregate = "SELECT Industry, COUNT(Id) TotalAccounts, AVG(AnnualRevenue) AvgRevenue \
                          FROM Account \
                          WHERE Industry != null \
                          GROUP BY Industry \
                          ORDER BY COUNT(Id) DESC \
                          LIMIT 10";

    let stats_result = client
        .query::<IndustryStats>(soql_aggregate)
        .await?;

    println!("Top 10 Industries by Account Count:\n");
    for (idx, stat) in stats_result.records.iter().enumerate() {
        let industry = stat.industry.as_deref().unwrap_or("(No Industry)");
        println!("{}. {}", idx + 1, industry);
        println!("   Accounts: {}", stat.total_accounts);
        if let Some(avg) = stat.avg_revenue {
            println!("   Avg Revenue: ${:.2}", avg);
        }
        println!();
    }

    // EXAMPLE 5: Query with date filters
    println!("═══ EXAMPLE 5: Query with Date Filters ═══");

    #[derive(Debug, Deserialize)]
    struct RecentAccount {
        #[serde(rename = "Id")]
        id: String,

        #[serde(rename = "Name")]
        name: String,

        #[serde(rename = "CreatedDate")]
        created_date: String,
    }

    let soql_dates = "SELECT Id, Name, CreatedDate \
                      FROM Account \
                      WHERE CreatedDate = LAST_N_DAYS:30 \
                      ORDER BY CreatedDate DESC \
                      LIMIT 5";

    let recent_result = client
        .query::<RecentAccount>(soql_dates)
        .await?;

    println!(
        "Accounts created in last 30 days: {}\n",
        recent_result.total_size
    );
    for account in &recent_result.records {
        println!("  • {} (Created: {})", account.name, account.created_date);
    }

    println!("\n═══ COMPLETE ═══");
    println!("Successfully demonstrated typed SOQL queries with pagination!");

    Ok(())
}
