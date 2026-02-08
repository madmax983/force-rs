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
use force::types::SalesforceId;
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
    let auth = ClientCredentials::new(client_id, client_secret);
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

    let account_id: SalesforceId = client.rest().create("Account", &account_data).await?;

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

    let contact_id: SalesforceId = client.rest().create("Contact", &contact_data).await?;

    println!("✓ Created Contact: {}\n", contact_id);

    // READ - Retrieve the Account
    println!("═══ READ ═══");
    let account = client.rest().get("Account", &account_id).await?;

    println!("Account Details:");
    println!("  ID: {}", account.get_field::<String>("Id")?);
    println!("  Name: {}", account.get_field::<String>("Name")?);
    println!("  Industry: {}", account.get_field::<String>("Industry")?);
    println!("  Website: {}", account.get_field::<String>("Website")?);

    // READ - Retrieve the Contact
    let contact = client.rest().get("Contact", &contact_id).await?;

    println!("\nContact Details:");
    println!("  ID: {}", contact.get_field::<String>("Id")?);
    println!(
        "  Name: {} {}",
        contact.get_field::<String>("FirstName")?,
        contact.get_field::<String>("LastName")?
    );
    println!("  Email: {}", contact.get_field::<String>("Email")?);
    println!("  Title: {}\n", contact.get_field::<String>("Title")?);

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
        updated_account.get_field::<String>("Industry")?
    );
    println!(
        "  Employees: {}",
        updated_account.get_field::<i32>("NumberOfEmployees")?
    );

    let updated_contact = client.rest().get("Contact", &contact_id).await?;

    println!("\nUpdated Contact:");
    println!("  Title: {}", updated_contact.get_field::<String>("Title")?);
    println!(
        "  Phone: {}\n",
        updated_contact.get_field::<String>("Phone")?
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

    match upsert_result_1 {
        force::api::rest::UpsertResult::Created(id) => {
            println!("✓ Created new contact: {}", id);
        }
        force::api::rest::UpsertResult::Updated(id) => {
            println!("✓ Updated existing contact: {}", id);
        }
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

    let jane_id = match upsert_result_2 {
        force::api::rest::UpsertResult::Created(id) => {
            println!("✓ Created new contact: {}\n", id);
            id
        }
        force::api::rest::UpsertResult::Updated(id) => {
            println!("✓ Updated existing contact: {}\n", id);
            id
        }
    };

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
