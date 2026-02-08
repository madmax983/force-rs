//! SOSL Search Example
//!
//! This example demonstrates multi-object text search using SOSL (Salesforce
//! Object Search Language).
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
//! cargo run --example search
//! ```

use force::auth::ClientCredentials;
use force::client::builder;

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

    // EXAMPLE 1: Basic text search across multiple objects
    println!("═══ EXAMPLE 1: Basic Multi-Object Search ═══");
    let sosl = "FIND {John} IN NAME FIELDS \
                RETURNING Account(Id, Name), Contact(Id, FirstName, LastName, Email)";

    let search_result = client.rest().search(sosl).await?;

    println!(
        "Search completed. Found results in {} object types\n",
        search_result.search_records.len()
    );

    for search_records in &search_result.search_records {
        let object_type = &search_records.attributes.type_;
        println!(
            "--- {} (found: {}) ---",
            object_type,
            search_records.records.len()
        );

        for record in &search_records.records {
            print!("  • ");

            match object_type.as_str() {
                "Account" => {
                    let id = record.get("Id").and_then(|v| v.as_str()).unwrap_or("");
                    let name = record.get("Name").and_then(|v| v.as_str()).unwrap_or("");
                    println!("{} (ID: {})", name, id);
                }
                "Contact" => {
                    let id = record.get("Id").and_then(|v| v.as_str()).unwrap_or("");
                    let first = record
                        .get("FirstName")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let last = record.get("LastName").and_then(|v| v.as_str()).unwrap_or("");
                    let email = record.get("Email").and_then(|v| v.as_str());

                    print!("{} {} (ID: {})", first, last, id);
                    if let Some(email) = email {
                        print!(" - {}", email);
                    }
                    println!();
                }
                _ => {
                    println!("Unknown object type");
                }
            }
        }
        println!();
    }

    // EXAMPLE 2: Search in specific fields
    println!("═══ EXAMPLE 2: Search in Specific Fields ═══");
    let sosl_email = "FIND {example.com} IN EMAIL FIELDS \
                      RETURNING Contact(Id, Name, Email, Phone), Lead(Id, Name, Email, Company)";

    let email_search = client.rest().search(sosl_email).await?;

    for search_records in &email_search.search_records {
        println!("--- {} ---", &search_records.attributes.type_);

        for record in &search_records.records {
            let name = record.get("Name").and_then(|v| v.as_str()).unwrap_or("");
            let email = record
                .get("Email")
                .and_then(|v| v.as_str())
                .unwrap_or("(no email)");

            println!("  • {} - {}", name, email);
        }
        println!();
    }

    // EXAMPLE 3: Search with filters and limits
    println!("═══ EXAMPLE 3: Search with Filters ═══");
    let sosl_filtered = "FIND {Technology} IN ALL FIELDS \
                         RETURNING Account(Id, Name, Industry WHERE Industry = 'Technology' \
                         ORDER BY Name LIMIT 5)";

    let filtered_search = client.rest().search(sosl_filtered).await?;

    for search_records in &filtered_search.search_records {
        println!(
            "--- {} ({}filtered and sorted) ---",
            &search_records.attributes.type_,
            search_records.records.len()
        );

        for (idx, record) in search_records.records.iter().enumerate() {
            let name = record.get("Name").and_then(|v| v.as_str()).unwrap_or("");
            let industry = record
                .get("Industry")
                .and_then(|v| v.as_str())
                .unwrap_or("(no industry)");

            println!("  {}. {} - {}", idx + 1, name, industry);
        }
        println!();
    }

    // EXAMPLE 4: Phone number search
    println!("═══ EXAMPLE 4: Phone Number Search ═══");
    let sosl_phone = "FIND {555-0100} IN PHONE FIELDS \
                      RETURNING Contact(Id, Name, Phone, MobilePhone), \
                      Account(Id, Name, Phone)";

    let phone_search = client.rest().search(sosl_phone).await?;

    println!("Found records with phone number '555-0100':\n");

    for search_records in &phone_search.search_records {
        if search_records.records.is_empty() {
            continue;
        }

        println!("--- {} ---", &search_records.attributes.type_);

        for record in &search_records.records {
            let name = record.get("Name").and_then(|v| v.as_str()).unwrap_or("");
            let phone = record.get("Phone").and_then(|v| v.as_str());
            let mobile = record.get("MobilePhone").and_then(|v| v.as_str());

            print!("  • {}", name);
            if let Some(p) = phone {
                print!(" | Phone: {}", p);
            }
            if let Some(m) = mobile {
                print!(" | Mobile: {}", m);
            }
            println!();
        }
        println!();
    }

    // EXAMPLE 5: Wildcard search
    println!("═══ EXAMPLE 5: Wildcard Search ═══");
    let sosl_wildcard = "FIND {Acme*} IN NAME FIELDS \
                         RETURNING Account(Id, Name), Opportunity(Id, Name, StageName)";

    let wildcard_search = client.rest().search(sosl_wildcard).await?;

    println!("Search for records starting with 'Acme':\n");

    for search_records in &wildcard_search.search_records {
        if search_records.records.is_empty() {
            continue;
        }

        println!(
            "--- {} (found: {}) ---",
            &search_records.attributes.type_,
            search_records.records.len()
        );

        for record in &search_records.records {
            let name = record.get("Name").and_then(|v| v.as_str()).unwrap_or("");

            if search_records.attributes.type_ == "Opportunity" {
                let stage = record
                    .get("StageName")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                println!("  • {} [{}]", name, stage);
            } else {
                println!("  • {}", name);
            }
        }
        println!();
    }

    // EXAMPLE 6: Search with division filter
    println!("═══ EXAMPLE 6: Advanced SOSL Features ═══");
    let sosl_advanced = "FIND {United} IN NAME FIELDS \
                         RETURNING Account(Id, Name, BillingCountry WHERE BillingCountry != null) \
                         LIMIT 10";

    let advanced_search = client.rest().search(sosl_advanced).await?;

    for search_records in &advanced_search.search_records {
        println!("--- {} ---", &search_records.attributes.type_);

        for record in &search_records.records {
            let name = record.get("Name").and_then(|v| v.as_str()).unwrap_or("");
            let country = record
                .get("BillingCountry")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown");

            println!("  • {} ({})", name, country);
        }
        println!();
    }

    // EXAMPLE 7: Count results by object type
    println!("═══ EXAMPLE 7: Result Summary ═══");
    let sosl_summary = "FIND {*} IN ALL FIELDS \
                        RETURNING Account(Id), Contact(Id), Lead(Id), Opportunity(Id)";

    let summary_search = client.rest().search(sosl_summary).await?;

    println!("Search Results Summary:\n");

    let mut total_results = 0;
    for search_records in &summary_search.search_records {
        let count = search_records.records.len();
        total_results += count;
        println!("  {}: {} results", &search_records.attributes.type_, count);
    }

    println!(
        "\n  Total: {} results across {} object types",
        total_results,
        summary_search.search_records.len()
    );

    println!("\n═══ COMPLETE ═══");
    println!("Successfully demonstrated SOSL search patterns!");

    Ok(())
}
