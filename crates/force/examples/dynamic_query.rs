//! Dynamic Query with DynamicSObject Example
//!
//! This example demonstrates how to work with dynamic, untyped queries using
//! DynamicSObject for maximum flexibility when field structure varies.
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
//! cargo run --example dynamic_query
//! ```

use force::auth::ClientCredentials;
use force::client::builder;
use force::types::DynamicSObject;

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

    // EXAMPLE 1: Basic dynamic query
    println!("═══ EXAMPLE 1: Basic Dynamic Query ═══");
    let soql = "SELECT Id, Name, Industry, Website FROM Account LIMIT 5";

    let result = client.query::<DynamicSObject>(soql).await?;

    println!("Found {} total accounts\n", result.total_size);

    for (idx, record) in result.records.iter().enumerate() {
        println!("{}. Record Type: {}", idx + 1, record.object_type());

        // Use get_field_as for typed field access
        if let Some(id) = record.get_field_as::<String>("Id")? {
            println!("   ID: {}", id);
        }
        if let Some(name) = record.get_field_as::<String>("Name")? {
            println!("   Name: {}", name);
        }

        // Handle optional fields gracefully
        if let Some(industry) = record.get_field_as::<String>("Industry")? {
            println!("   Industry: {}", industry);
        }

        if let Some(website) = record.get_field_as::<String>("Website")? {
            println!("   Website: {}", website);
        }
        println!();
    }

    // EXAMPLE 2: Inspect available fields
    println!("═══ EXAMPLE 2: Field Introspection ═══");
    let soql2 = "SELECT Id, Name, Email, Phone, Title FROM Contact LIMIT 1";

    let result2 = client.query::<DynamicSObject>(soql2).await?;

    if let Some(first_contact) = result2.records.first() {
        println!("Object Type: {}", first_contact.object_type());
        println!("Available fields:");

        for field_name in first_contact.field_names() {
            println!("  - {}", field_name);
        }
        println!();

        // Check for specific fields
        if first_contact.has_field("Email") {
            println!("✓ Email field is present");
        }

        if !first_contact.has_field("Fax") {
            println!("✗ Fax field not queried");
        }
    }

    // EXAMPLE 3: Working with different field types
    println!("\n═══ EXAMPLE 3: Different Field Types ═══");
    let soql3 = "SELECT Id, Name, NumberOfEmployees, AnnualRevenue, IsDeleted, CreatedDate \
                 FROM Account \
                 LIMIT 3";

    let result3 = client.query::<DynamicSObject>(soql3).await?;

    for record in &result3.records {
        if let Some(name) = record.get_field_as::<String>("Name")? {
            println!("Account: {}", name);
        }

        // Integer field
        if let Some(employees) = record.get_field_as::<i32>("NumberOfEmployees")? {
            println!("  Employees: {}", employees);
        }

        // Float field
        if let Some(revenue) = record.get_field_as::<f64>("AnnualRevenue")? {
            println!("  Revenue: ${:.2}", revenue);
        }

        // Boolean field
        if let Some(is_deleted) = record.get_field_as::<bool>("IsDeleted")? {
            println!("  Deleted: {}", is_deleted);
        }

        // Date field (as string)
        if let Some(created) = record.get_field_as::<String>("CreatedDate")? {
            println!("  Created: {}", created);
        }
        println!();
    }

    // EXAMPLE 4: Handling null values
    println!("═══ EXAMPLE 4: Null Value Handling ═══");
    let soql4 = "SELECT Id, Name, Description, Website FROM Account LIMIT 5";

    let result4 = client.query::<DynamicSObject>(soql4).await?;

    for record in &result4.records {
        if let Some(name) = record.get_field_as::<String>("Name")? {
            println!("Account: {}", name);
        }

        // Option pattern for nullable fields
        match record.get_field_as::<String>("Description")? {
            Some(desc) => println!("  Description: {}", desc),
            None => println!("  Description: (none)"),
        }

        match record.get_field_as::<String>("Website")? {
            Some(web) => println!("  Website: {}", web),
            None => println!("  Website: (none)"),
        }
        println!();
    }

    // EXAMPLE 5: Dynamic query with complex data
    println!("═══ EXAMPLE 5: Complex Nested Data ═══");
    let soql5 = "SELECT Id, Name, \
                 (SELECT Id, FirstName, LastName, Email FROM Contacts LIMIT 3) \
                 FROM Account \
                 WHERE Id IN (SELECT AccountId FROM Contact) \
                 LIMIT 2";

    let result5 = client.query::<DynamicSObject>(soql5).await?;

    for account in &result5.records {
        if let Some(account_name) = account.get_field_as::<String>("Name")? {
            println!("Account: {}", account_name);
        }

        // Access nested subquery results using get_field (returns Option<&Value>)
        if let Some(contacts_value) = account.get_field("Contacts") {
            if let Some(contacts_obj) = contacts_value.as_object() {
                if let Some(records) = contacts_obj.get("records") {
                    if let Some(records_array) = records.as_array() {
                        println!("  Contacts: {}", records_array.len());

                        for contact in records_array {
                            if let Some(first_name) =
                                contact.get("FirstName").and_then(|v| v.as_str())
                            {
                                print!("    - {}", first_name);
                            }
                            if let Some(last_name) =
                                contact.get("LastName").and_then(|v| v.as_str())
                            {
                                print!(" {}", last_name);
                            }
                            println!();
                        }
                    }
                }
            }
        }
        println!();
    }

    // EXAMPLE 6: Error handling for missing fields
    println!("═══ EXAMPLE 6: Error Handling ═══");
    let soql6 = "SELECT Id, Name FROM Account LIMIT 1";

    let result6 = client.query::<DynamicSObject>(soql6).await?;

    if let Some(record) = result6.records.first() {
        // This will work
        match record.get_field_as::<String>("Name") {
            Ok(Some(name)) => println!("✓ Successfully got Name: {}", name),
            Ok(None) => println!("✗ Name field is null"),
            Err(e) => println!("✗ Error getting Name: {}", e),
        }

        // This will return None - field not in query
        match record.get_field_as::<String>("Industry") {
            Ok(Some(industry)) => println!("✓ Industry: {}", industry),
            Ok(None) => println!("✓ Industry field not present (as expected)"),
            Err(e) => println!("✗ Error deserializing Industry: {}", e),
        }

        // Safe way to check for fields
        if record.has_field("Industry") {
            if let Some(industry) = record.get_field_as::<String>("Industry")? {
                println!("Industry: {}", industry);
            }
        } else {
            println!("✓ Industry field not present (as expected)");
        }
    }

    // EXAMPLE 7: Collect and process with iterators
    println!("\n═══ EXAMPLE 7: Functional Processing ═══");
    let soql7 = "SELECT Id, Name, AnnualRevenue FROM Account \
                 WHERE AnnualRevenue != null \
                 LIMIT 10";

    let result7 = client.query::<DynamicSObject>(soql7).await?;

    // Collect revenue values
    let revenues: Vec<f64> = result7
        .records
        .iter()
        .filter_map(|record| record.get_field_as::<f64>("AnnualRevenue").ok().flatten())
        .collect();

    if !revenues.is_empty() {
        let total: f64 = revenues.iter().sum();
        let avg: f64 = total / revenues.len() as f64;
        let max: f64 = revenues.iter().cloned().fold(f64::MIN, f64::max);
        let min: f64 = revenues.iter().cloned().fold(f64::MAX, f64::min);

        println!("Revenue Statistics:");
        println!("  Count: {}", revenues.len());
        println!("  Total: ${:.2}", total);
        println!("  Average: ${:.2}", avg);
        println!("  Max: ${:.2}", max);
        println!("  Min: ${:.2}", min);
    }

    println!("\n═══ COMPLETE ═══");
    println!("Successfully demonstrated dynamic query patterns!");

    Ok(())
}
