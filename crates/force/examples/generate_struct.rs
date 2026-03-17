//! Experimental feature: Struct Generator
//! Requires the `schema` feature: `force = { version = "0.1", features = ["schema"] }`

#![allow(clippy::too_many_lines)]

use force::auth::ClientCredentials;
use force::client::ForceClientBuilder;
use std::env;

#[cfg(feature = "schema")]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    use force::experimental::StructGenerator;

    let client_id =
        env::var("SALESFORCE_CLIENT_ID").unwrap_or_else(|_| "your-client-id".to_string());
    let client_secret =
        env::var("SALESFORCE_CLIENT_SECRET").unwrap_or_else(|_| "your-client-secret".to_string());
    let sobject_name = env::args().nth(1).unwrap_or_else(|| "Account".to_string());

    println!("Authenticating with Salesforce...");

    // Authenticate with OAuth 2.0 client credentials
    // For Sandbox, use: ClientCredentials::new_sandbox("client-id", "client-secret")?
    let auth = ClientCredentials::new_production(&client_id, &client_secret)?;

    // We expect this to fail gracefully if the credentials are dummy, but in a real org
    // this will retrieve the describe and generate real code.
    let client = match ForceClientBuilder::new().authenticate(auth).build().await {
        Ok(c) => {
            // Test the connection
            if let Err(e) = c.rest().describe(&sobject_name).await {
                println!(
                    "Failed to retrieve describe metadata: {e}. Are your credentials correct?"
                );
                return fallback_demonstration();
            }
            c
        }
        Err(e) => {
            println!(
                "Failed to authenticate: {e}. Please set SALESFORCE_CLIENT_ID and SALESFORCE_CLIENT_SECRET to valid credentials."
            );
            return fallback_demonstration();
        }
    };

    println!("Retrieving describe metadata for {sobject_name}...");
    let describe = client.rest().describe(&sobject_name).await?;

    println!("\n==========================================");
    println!("GENERATED RUST CODE FOR {}", sobject_name.to_uppercase());
    println!("==========================================\n");

    let generated_code = StructGenerator::generate(&describe);
    println!("{generated_code}");

    Ok(())
}

#[cfg(feature = "schema")]
fn fallback_demonstration() -> anyhow::Result<()> {
    use force::api::rest::SObjectDescribe;
    use force::experimental::StructGenerator;

    println!("\nFallback demonstration mode:\n");

    // Generate some dummy struct to show functionality
    let json = r#"{
        "activateable": false,
        "createable": true,
        "custom": false,
        "customSetting": false,
        "deletable": true,
        "deprecatedAndHidden": false,
        "feedEnabled": true,
        "hasSubtypes": false,
        "isSubtype": false,
        "keyPrefix": "003",
        "label": "Contact Object",
        "labelPlural": "Contacts",
        "layoutable": true,
        "mergeable": true,
        "mruEnabled": true,
        "name": "Contact",
        "queryable": true,
        "replicateable": true,
        "retrieveable": true,
        "searchable": true,
        "triggerable": true,
        "undeletable": true,
        "updateable": true,
        "urls": {
            "sobject": "/services/data/v60.0/sobjects/Contact"
        },
        "fields": [
            {
                "aggregatable": true,
                "autoNumber": false,
                "byteLength": 18,
                "calculated": false,
                "cascadeDelete": false,
                "caseSensitive": false,
                "createable": false,
                "custom": false,
                "defaultedOnCreate": true,
                "dependentPicklist": false,
                "deprecatedAndHidden": false,
                "digits": 0,
                "displayLocationInDecimal": false,
                "encrypted": false,
                "externalId": false,
                "filterable": true,
                "groupable": true,
                "highScaleNumber": false,
                "htmlFormatted": false,
                "idLookup": true,
                "label": "Contact ID",
                "length": 18,
                "name": "Id",
                "nameField": false,
                "namePointing": false,
                "nillable": false,
                "permissionable": false,
                "polymorphicForeignKey": false,
                "precision": 0,
                "queryByDistance": false,
                "referenceTo": [],
                "restrictedDelete": false,
                "restrictedPicklist": false,
                "scale": 0,
                "soapType": "tns:ID",
                "sortable": true,
                "type": "id",
                "unique": false,
                "updateable": false,
                "writeRequiresMasterRead": false
            },
            {
                "aggregatable": true,
                "autoNumber": false,
                "byteLength": 255,
                "calculated": false,
                "cascadeDelete": false,
                "caseSensitive": false,
                "createable": true,
                "custom": false,
                "defaultedOnCreate": true,
                "dependentPicklist": false,
                "deprecatedAndHidden": false,
                "digits": 0,
                "displayLocationInDecimal": false,
                "encrypted": false,
                "externalId": false,
                "filterable": true,
                "groupable": true,
                "highScaleNumber": false,
                "htmlFormatted": false,
                "idLookup": false,
                "label": "First Name",
                "length": 255,
                "name": "FirstName",
                "nameField": true,
                "namePointing": false,
                "nillable": true,
                "permissionable": false,
                "polymorphicForeignKey": false,
                "precision": 0,
                "queryByDistance": false,
                "referenceTo": [],
                "restrictedDelete": false,
                "restrictedPicklist": false,
                "scale": 0,
                "soapType": "xsd:string",
                "sortable": true,
                "type": "string",
                "unique": false,
                "updateable": true,
                "writeRequiresMasterRead": false
            }
        ],
        "childRelationships": [],
        "recordTypeInfos": []
    }"#;

    let describe: SObjectDescribe = serde_json::from_str(json)?;
    println!("{}", StructGenerator::generate(&describe));
    Ok(())
}

#[cfg(not(feature = "schema"))]
fn main() {
    println!("This example requires the 'schema' feature. Run with:");
    println!("cargo run --example generate_struct --features schema");
}
