//! One-shot profiling harness for `DataSeeder::seed` (mock-data generation +
//! Composite Batch insert).
//!
//! This is not a criterion benchmark: it is a single realistic run of the
//! public `DataSeeder::seed` entry point, meant to be executed once under
//! `valgrind --tool=callgrind` (instruction counts) and
//! `valgrind --tool=dhat` (allocation counts) so those tools attribute cost
//! to real call stacks instead of a criterion harness loop.
//!
//! Workload: seed 5,000 `Contact`-shaped mock records (15 creatable fields
//! spanning the common `FieldType` variants: string, textarea, int, double,
//! boolean, date, datetime, time, email, phone, url, picklist) through
//! `DataSeeder::seed`, which drives `generate_mock_record` per record and
//! flushes every 25 records (the Salesforce Composite Batch limit) against
//! an in-process mock Composite Batch endpoint. This is the same public path
//! a caller uses to populate a sandbox with fixture data; record count and
//! shape are fixed (no RNG) so repeated runs are byte-for-byte deterministic,
//! which is required for callgrind/dhat comparisons to be meaningful.
//!
//! Run directly:
//! ```bash
//! cargo run --release -p force --features composite --bench data_seeder_profile
//! ```
//!
//! Profile (this is a `[[bench]]` target, not a `[[bin]]`: the compiled
//! executable lands under `target/release/deps/`, not `target/release/`):
//! ```bash
//! CARGO_PROFILE_RELEASE_DEBUG=true cargo build --release -p force --features composite --bench data_seeder_profile
//! BIN=$(find target/release/deps -maxdepth 1 -name 'data_seeder_profile-*' -executable -not -name '*.d')
//! valgrind --tool=callgrind --callgrind-out-file=callgrind.out "$BIN"
//! callgrind_annotate callgrind.out
//!
//! valgrind --tool=dhat --dhat-out-file=dhat.out "$BIN"
//! ```
#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

use force::auth::{AccessToken, Authenticator, TokenResponse};
use force::client::builder;
use force::data::DataSeeder;
use force::error::Result as ForceResult;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Number of records seeded through `DataSeeder::seed` in one profiling run.
const RECORD_COUNT: usize = 5_000;

#[derive(Debug, Clone)]
struct StaticAuthenticator {
    token: String,
    instance_url: String,
}

#[async_trait::async_trait]
impl Authenticator for StaticAuthenticator {
    async fn authenticate(&self) -> ForceResult<AccessToken> {
        Ok(AccessToken::from_response(TokenResponse {
            access_token: secrecy::SecretString::new(self.token.clone().into()),
            instance_url: self.instance_url.clone(),
            token_type: "Bearer".to_string(),
            issued_at: "1704067200000".to_string(),
            signature: "profile-sig".to_string(),
            expires_in: Some(7200),
            refresh_token: None,
        }))
    }

    async fn refresh(&self) -> ForceResult<AccessToken> {
        self.authenticate().await
    }
}

/// A `Contact`-shaped describe: 15 creatable fields spanning the common
/// `FieldType` variants a real org describe returns.
fn contact_describe_json() -> serde_json::Value {
    fn field(name: &str, ty: &str, label: &str) -> serde_json::Value {
        serde_json::json!({
            "name": name, "type": ty, "label": label, "createable": true,
            "autoNumber": false, "calculated": false,
            "aggregatable": true, "byteLength": 255,
            "cascadeDelete": false, "caseSensitive": false, "custom": false,
            "defaultedOnCreate": false, "dependentPicklist": false, "deprecatedAndHidden": false,
            "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
            "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
            "idLookup": false, "length": 255, "nameField": false, "namePointing": false, "nillable": true,
            "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
            "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "xsd:string",
            "sortable": true, "unique": false, "updateable": true, "writeRequiresMasterRead": false,
            "referenceTo": []
        })
    }

    serde_json::json!({
        "name": "Contact", "label": "Contact", "custom": false, "queryable": true,
        "activateable": false, "createable": true, "customSetting": false, "deletable": true,
        "deprecatedAndHidden": false, "feedEnabled": true, "hasSubtypes": false,
        "isSubtype": false, "keyPrefix": "003", "labelPlural": "Contacts", "layoutable": true,
        "mergeable": true, "mruEnabled": true, "replicateable": true, "retrieveable": true,
        "searchable": true, "triggerable": true, "undeletable": true, "updateable": true,
        "urls": {}, "childRelationships": [], "recordTypeInfos": [],
        "fields": [
            field("FirstName", "string", "First Name"),
            field("LastName", "string", "Last Name"),
            field("Email", "email", "Email"),
            field("Phone", "phone", "Phone"),
            field("MobilePhone", "phone", "Mobile Phone"),
            field("Title", "string", "Title"),
            field("Department", "string", "Department"),
            field("Description", "textarea", "Description"),
            field("MailingCity", "string", "Mailing City"),
            field("MailingState", "string", "Mailing State"),
            field("Birthdate", "date", "Birthdate"),
            field("LastActivityDate", "date", "Last Activity Date"),
            field("HasOptedOutOfEmail", "boolean", "Email Opt Out"),
            field("LeadSource", "picklist", "Lead Source"),
            field("Website", "url", "Website"),
        ]
    })
}

async fn mount_endpoints(mock_server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/services/data/v67.0/sobjects/Contact/describe"))
        .respond_with(ResponseTemplate::new(200).set_body_json(contact_describe_json()))
        .mount(mock_server)
        .await;

    let batch_results: Vec<_> = (0..25)
        .map(|i| {
            serde_json::json!({
                "statusCode": 201,
                "result": { "id": format!("003xx00000000{i:04}AAA"), "success": true, "errors": [] }
            })
        })
        .collect();

    Mock::given(method("POST"))
        .and(path("/services/data/v67.0/composite/batch"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "hasErrors": false,
            "results": batch_results
        })))
        .mount(mock_server)
        .await;
}

#[tokio::main]
async fn main() {
    let mock_server = MockServer::start().await;
    mount_endpoints(&mock_server).await;

    let auth = StaticAuthenticator {
        token: "profile-token".to_string(),
        instance_url: mock_server.uri(),
    };
    let client = builder()
        .authenticate(auth)
        .build()
        .await
        .expect("failed to build profiling client");

    let seeder = DataSeeder::new(&client);
    let success_count: usize = seeder
        .seed("Contact", RECORD_COUNT)
        .await
        .expect("DataSeeder run failed");

    // Keep the result observable so the compiler can't fold the whole run away.
    println!("success_count={success_count}");
}
