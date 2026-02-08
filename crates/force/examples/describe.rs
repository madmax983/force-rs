//! Schema Introspection with Describe API Example
//!
//! This example demonstrates how to use the Describe API to introspect
//! Salesforce object metadata, fields, and relationships.
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
//! cargo run --example describe
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
    let auth = ClientCredentials::new(client_id, client_secret);
    let client = builder().authenticate(auth).build().await?;
    println!("✓ Authentication successful\n");

    // EXAMPLE 1: Global Describe - List all objects
    println!("═══ EXAMPLE 1: Global Describe ═══");
    let global_describe = client.rest().describe_global().await?;

    println!(
        "Organization has {} objects",
        global_describe.sobjects.len()
    );

    // Filter and display standard objects
    let standard_objects: Vec<_> = global_describe
        .sobjects
        .iter()
        .filter(|obj| !obj.custom && obj.createable && obj.queryable)
        .take(10)
        .collect();

    println!("\nStandard objects (createable and queryable, first 10):");
    for obj in standard_objects {
        println!("  • {} ({})", obj.label, obj.name);
        println!(
            "    Updateable: {}, Deletable: {}",
            obj.updateable, obj.deleteable
        );
    }

    // Filter and display custom objects
    let custom_objects: Vec<_> = global_describe
        .sobjects
        .iter()
        .filter(|obj| obj.custom)
        .take(5)
        .collect();

    if !custom_objects.is_empty() {
        println!("\nCustom objects (first 5):");
        for obj in custom_objects {
            println!("  • {} ({})", obj.label, obj.name);
        }
    } else {
        println!("\nNo custom objects found in this org");
    }

    // EXAMPLE 2: Describe specific object - Account
    println!("\n═══ EXAMPLE 2: Describe Account Object ═══");
    let account_describe = client.rest().describe("Account").await?;

    println!(
        "Object: {} ({})",
        account_describe.label, account_describe.name
    );
    println!("Label Plural: {}", account_describe.label_plural);
    println!("Key Prefix: {}", account_describe.key_prefix);
    println!("\nPermissions:");
    println!("  Createable: {}", account_describe.createable);
    println!("  Updateable: {}", account_describe.updateable);
    println!("  Deleteable: {}", account_describe.deleteable);
    println!("  Queryable: {}", account_describe.queryable);
    println!("  Searchable: {}", account_describe.searchable);

    println!("\nTotal fields: {}", account_describe.fields.len());

    // Display required fields
    let required_fields: Vec<_> = account_describe
        .fields
        .iter()
        .filter(|f| !f.nillable && f.createable && !f.defaulted_on_create)
        .collect();

    if !required_fields.is_empty() {
        println!("\nRequired fields for creation:");
        for field in required_fields {
            println!(
                "  • {} ({}) - {}",
                field.label, field.name, field.field_type
            );
        }
    }

    // Display commonly used fields
    println!("\nCommonly used fields:");
    let common_field_names = ["Name", "Id", "OwnerId", "CreatedDate", "LastModifiedDate"];

    for field_name in &common_field_names {
        if let Some(field) = account_describe
            .fields
            .iter()
            .find(|f| &f.name == field_name)
        {
            println!("  • {} ({})", field.label, field.name);
            println!("    Type: {}, Length: {:?}", field.field_type, field.length);
            println!(
                "    Updateable: {}, Nillable: {}",
                field.updateable, field.nillable
            );
        }
    }

    // EXAMPLE 3: Describe Contact with relationships
    println!("\n═══ EXAMPLE 3: Contact Relationships ═══");
    let contact_describe = client.rest().describe("Contact").await?;

    println!(
        "Object: {} ({})",
        contact_describe.label, contact_describe.name
    );

    // Find relationship fields
    let relationship_fields: Vec<_> = contact_describe
        .fields
        .iter()
        .filter(|f| f.field_type == "reference")
        .collect();

    println!(
        "\nRelationship fields ({} total):",
        relationship_fields.len()
    );
    for field in relationship_fields.iter().take(10) {
        print!("  • {} ({}) → ", field.label, field.name);

        if let Some(ref_to) = &field.reference_to {
            if ref_to.len() == 1 {
                println!("{}", ref_to[0]);
            } else {
                println!("{:?}", ref_to);
            }
        } else {
            println!("(unknown)");
        }
    }

    // Child relationships
    if let Some(child_relationships) = &contact_describe.child_relationships {
        println!(
            "\nChild relationships ({} total):",
            child_relationships.len()
        );
        for child_rel in child_relationships.iter().take(5) {
            println!("  • {} (via {})", child_rel.child_sobject, child_rel.field);
            if let Some(rel_name) = &child_rel.relationship_name {
                println!("    Relationship name: {}", rel_name);
            }
        }
    }

    // EXAMPLE 4: Picklist field values
    println!("\n═══ EXAMPLE 4: Picklist Values ═══");
    let opportunity_describe = client.rest().describe("Opportunity").await?;

    // Find StageName field (picklist)
    if let Some(stage_field) = opportunity_describe
        .fields
        .iter()
        .find(|f| f.name == "StageName")
    {
        println!("Field: {} ({})", stage_field.label, stage_field.name);
        println!("Type: {}", stage_field.field_type);

        if let Some(picklist_values) = &stage_field.picklist_values {
            println!("\nAvailable stage values:");
            for (idx, value) in picklist_values.iter().enumerate() {
                let status = if value.active { "active" } else { "inactive" };
                let default = if value.default_value {
                    " [DEFAULT]"
                } else {
                    ""
                };
                println!("  {}. {} ({}){}", idx + 1, value.label, status, default);
            }
        }
    }

    // Find Type field (picklist)
    if let Some(type_field) = opportunity_describe
        .fields
        .iter()
        .find(|f| f.name == "Type")
    {
        println!("\nField: {} ({})", type_field.label, type_field.name);

        if let Some(picklist_values) = &type_field.picklist_values {
            println!("Available type values:");
            for value in picklist_values {
                if value.active {
                    println!("  • {}", value.label);
                }
            }
        }
    }

    // EXAMPLE 5: Field-level security and validation
    println!("\n═══ EXAMPLE 5: Field Properties ═══");
    let case_describe = client.rest().describe("Case").await?;

    println!("Analyzing Case object fields:\n");

    // Required fields
    let required: Vec<_> = case_describe
        .fields
        .iter()
        .filter(|f| !f.nillable && f.createable)
        .map(|f| f.label.as_str())
        .collect();
    println!("Required fields: {}", required.join(", "));

    // Auto-populated fields
    let auto_populated: Vec<_> = case_describe
        .fields
        .iter()
        .filter(|f| f.defaulted_on_create)
        .map(|f| f.label.as_str())
        .collect();
    if !auto_populated.is_empty() {
        println!("Auto-populated: {}", auto_populated.join(", "));
    }

    // Unique fields
    let unique: Vec<_> = case_describe
        .fields
        .iter()
        .filter(|f| f.unique)
        .map(|f| f.label.as_str())
        .collect();
    if !unique.is_empty() {
        println!("Unique fields: {}", unique.join(", "));
    }

    // EXAMPLE 6: Custom field detection
    println!("\n═══ EXAMPLE 6: Custom Fields ═══");

    // Check if there are custom fields on Account
    let custom_fields: Vec<_> = account_describe
        .fields
        .iter()
        .filter(|f| f.custom)
        .collect();

    if !custom_fields.is_empty() {
        println!("Custom fields on Account ({} total):", custom_fields.len());
        for field in custom_fields.iter().take(10) {
            println!(
                "  • {} ({}) - {}",
                field.label, field.name, field.field_type
            );
            if let Some(help) = &field.inline_help_text {
                println!("    Help: {}", help);
            }
        }
    } else {
        println!("No custom fields found on Account object");
    }

    // EXAMPLE 7: Generate SOQL query template
    println!("\n═══ EXAMPLE 7: Generate SOQL Template ═══");

    // Get queryable, accessible fields from Contact
    let queryable_fields: Vec<_> = contact_describe
        .fields
        .iter()
        .filter(|f| !f.name.starts_with("Is") && f.name != "Id") // Skip boolean and Id for brevity
        .filter(|f| match f.field_type.as_str() {
            "address" | "location" | "base64" => false, // Skip complex types
            _ => true,
        })
        .take(15)
        .map(|f| f.name.as_str())
        .collect();

    let query_template = format!(
        "SELECT Id, {}\nFROM Contact\nWHERE CreatedDate = LAST_N_DAYS:30\nLIMIT 100",
        queryable_fields.join(", ")
    );

    println!("Sample SOQL query template:\n");
    println!("{}", query_template);

    // EXAMPLE 8: Object capabilities summary
    println!("\n═══ EXAMPLE 8: Object Capabilities Summary ═══");

    let objects_to_check = ["Account", "Contact", "Lead", "Opportunity", "Case"];

    println!("Object capabilities:");
    println!(
        "{:<15} {:>6} {:>6} {:>6} {:>6} {:>6}",
        "Object", "Create", "Read", "Update", "Delete", "Search"
    );
    println!("{}", "-".repeat(60));

    for object_name in &objects_to_check {
        let describe = client.rest().describe(object_name).await?;

        println!(
            "{:<15} {:>6} {:>6} {:>6} {:>6} {:>6}",
            describe.name,
            if describe.createable { "✓" } else { "✗" },
            if describe.queryable { "✓" } else { "✗" },
            if describe.updateable { "✓" } else { "✗" },
            if describe.deleteable { "✓" } else { "✗" },
            if describe.searchable { "✓" } else { "✗" },
        );
    }

    println!("\n═══ COMPLETE ═══");
    println!("Successfully demonstrated Describe API patterns!");

    Ok(())
}
