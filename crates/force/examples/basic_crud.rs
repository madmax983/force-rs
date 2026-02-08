//! Basic CRUD Operations Example
//!
//! This example demonstrates the full lifecycle of creating, reading, updating,
//! and deleting records using the REST API.
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
//! cargo run --example basic_crud
//! ```

use force::auth::ClientCredentials;
use force::client::builder;
use serde_json::json;

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

    // CREATE - Insert a new Account
    println!("═══ CREATE ═══");
    let account_data = json!({
        "Name": "Acme Corporation",
        "Industry": "Technology",
        "Website": "https://acme.example.com",
        "NumberOfEmployees": 500,
        "Description": "Leading provider of innovative solutions"
    });

    let account_response = client.rest().create("Account", &account_data).await?;
    let account_id = account_response.id.expect("Account ID should be present");

    println!("✓ Created Account: {}", account_id);

    // CREATE - Insert a related Contact
    let contact_data = json!({
        "FirstName": "John",
        "LastName": "Doe",
        "Email": "john.doe@acme.example.com",
        "Phone": "+1-555-0100",
        "AccountId": account_id.to_string(),
        "Title": "Chief Technology Officer"
    });

    let contact_response = client.rest().create("Contact", &contact_data).await?;
    let contact_id = contact_response.id.expect("Contact ID should be present");

    println!("✓ Created Contact: {}\n", contact_id);

    // READ - Retrieve the Account
    println!("═══ READ ═══");
    let account = client.rest().get("Account", &account_id).await?;

    println!("Account Details:");
    println!("  ID: {}", account["Id"].as_str().unwrap_or("N/A"));
    println!("  Name: {}", account["Name"].as_str().unwrap_or("N/A"));
    println!("  Industry: {}", account["Industry"].as_str().unwrap_or("N/A"));
    println!("  Website: {}", account["Website"].as_str().unwrap_or("N/A"));

    // READ - Retrieve the Contact
    let contact = client.rest().get("Contact", &contact_id).await?;

    println!("\nContact Details:");
    println!("  ID: {}", contact["Id"].as_str().unwrap_or("N/A"));
    println!(
        "  Name: {} {}",
        contact["FirstName"].as_str().unwrap_or("N/A"),
        contact["LastName"].as_str().unwrap_or("N/A")
    );
    println!("  Email: {}", contact["Email"].as_str().unwrap_or("N/A"));
    println!("  Title: {}\n", contact["Title"].as_str().unwrap_or("N/A"));

    // UPDATE - Modify the Account
    println!("═══ UPDATE ═══");
    let account_update = json!({
        "Industry": "Software",
        "NumberOfEmployees": 750,
        "Description": "Global leader in enterprise software solutions"
    });

    client
        .rest()
        .update("Account", &account_id, &account_update)
        .await?;

    println!("✓ Updated Account (Industry, Employees, Description)");

    // UPDATE - Modify the Contact
    let contact_update = json!({
        "Title": "Senior Vice President of Technology",
        "Phone": "+1-555-0101"
    });

    client
        .rest()
        .update("Contact", &contact_id, &contact_update)
        .await?;

    println!("✓ Updated Contact (Title, Phone)\n");

    // READ - Verify updates
    println!("═══ VERIFY UPDATES ═══");
    let updated_account = client.rest().get("Account", &account_id).await?;

    println!("Updated Account:");
    println!(
        "  Industry: {}",
        updated_account["Industry"].as_str().unwrap_or("N/A")
    );
    println!(
        "  Employees: {}",
        updated_account["NumberOfEmployees"].as_i64().unwrap_or(0)
    );

    let updated_contact = client.rest().get("Contact", &contact_id).await?;

    println!("\nUpdated Contact:");
    println!("  Title: {}", updated_contact["Title"].as_str().unwrap_or("N/A"));
    println!(
        "  Phone: {}\n",
        updated_contact["Phone"].as_str().unwrap_or("N/A")
    );

    // UPSERT - Update or insert using external ID
    println!("═══ UPSERT ═══");

    // First upsert - will update existing contact by email
    let upsert_data_1 = json!({
        "FirstName": "John",
        "LastName": "Doe",
        "Title": "Chief Technology Officer",
        "Department": "Engineering"
    });

    let upsert_result_1 = client
        .rest()
        .upsert(
            "Contact",
            "Email",
            "john.doe@acme.example.com",
            &upsert_data_1,
        )
        .await?;

    if upsert_result_1.is_created() {
        println!("✓ Created new contact: {}", upsert_result_1.id);
    } else {
        println!("✓ Updated existing contact: {}", upsert_result_1.id);
    }

    // Second upsert - will create new contact (different email)
    let upsert_data_2 = json!({
        "FirstName": "Jane",
        "LastName": "Smith",
        "Email": "jane.smith@acme.example.com",
        "Title": "VP of Engineering",
        "AccountId": account_id.to_string()
    });

    let upsert_result_2 = client
        .rest()
        .upsert(
            "Contact",
            "Email",
            "jane.smith@acme.example.com",
            &upsert_data_2,
        )
        .await?;

    let jane_id = upsert_result_2.id.clone();
    if upsert_result_2.is_created() {
        println!("✓ Created new contact: {}\n", jane_id);
    } else {
        println!("✓ Updated existing contact: {}\n", jane_id);
    }

    // DELETE - Remove records (cleanup)
    println!("═══ DELETE (Cleanup) ═══");

    // Delete contacts first (child records)
    client.rest().delete("Contact", &contact_id).await?;
    println!("✓ Deleted Contact: {}", contact_id);

    client.rest().delete("Contact", &jane_id).await?;
    println!("✓ Deleted Contact: {}", jane_id);

    // Delete account (parent record)
    client.rest().delete("Account", &account_id).await?;
    println!("✓ Deleted Account: {}", account_id);

    println!("\n═══ COMPLETE ═══");
    println!("Successfully demonstrated full CRUD lifecycle!");

    Ok(())
}
