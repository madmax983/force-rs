//! One-shot profiling harness for `RelationalSeeder::seed_hierarchy`
//! (mock-data generation + Composite Graph insert of a parent + N children).
//!
//! This is not a criterion benchmark: it is a single realistic run of the
//! public `RelationalSeeder::seed_hierarchy` entry point, meant to be
//! executed once under `valgrind --tool=callgrind` (instruction counts) and
//! `valgrind --tool=dhat` (allocation counts) so those tools attribute cost
//! to real call stacks instead of a criterion harness loop.
//!
//! Workload: seed one `Account`-shaped parent record and 400 `Contact`-shaped
//! child records (15 creatable fields each, spanning the common `FieldType`
//! variants: string, textarea, int, double, boolean, date, datetime, time,
//! email, phone, url, picklist, reference) through
//! `RelationalSeeder::seed_hierarchy`, which drives `generate_mock_record`
//! per record and submits the whole hierarchy as a single Composite Graph
//! request (capped at 500 total subrequests by the API) against an
//! in-process mock endpoint. This is the same public path a caller uses to
//! populate a sandbox with a realistic parent/child fixture in one call.
//! Record count and shape are fixed (no RNG) so repeated runs are
//! byte-for-byte deterministic, which is required for callgrind/dhat
//! comparisons to be meaningful.
//!
//! Run directly (`cargo run` doesn't support `--bench`, so use `cargo bench`):
//! ```bash
//! cargo bench -p force --features data_utility,composite_graph --bench relational_seeder_profile
//! ```
//!
//! Profile (this is a `[[bench]]` target, not a `[[bin]]`: the compiled
//! executable lands under `target/release/deps/`, not `target/release/`):
//! ```bash
//! CARGO_PROFILE_RELEASE_DEBUG=true cargo build --release -p force --features data_utility,composite_graph --bench relational_seeder_profile
//! BIN=$(find target/release/deps -maxdepth 1 -name 'relational_seeder_profile-*' -executable -not -name '*.d')
//! valgrind --tool=callgrind --callgrind-out-file=callgrind.out "$BIN"
//! callgrind_annotate callgrind.out
//!
//! valgrind --tool=dhat --dhat-out-file=dhat.out "$BIN"
//! ```
#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

use force::auth::{AccessToken, Authenticator, TokenResponse};
use force::client::builder;
use force::data::RelationalSeeder;
use force::error::Result as ForceResult;
use force::types::SObjectDescribe;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Number of child records seeded per parent in one profiling run. Kept
/// under the Composite Graph API's 500-subrequest cap (1 parent + N children).
const CHILD_COUNT: usize = 400;

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

fn field(name: &str, ty: &str, label: &str, ref_to: &[&str]) -> serde_json::Value {
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
        "referenceTo": ref_to
    })
}

/// An `Account`-shaped describe: a handful of creatable fields, no
/// relationships (it is the parent).
fn account_describe() -> SObjectDescribe {
    let describe_json = serde_json::json!({
        "name": "Account", "label": "Account", "custom": false, "queryable": true,
        "activateable": false, "createable": true, "customSetting": false, "deletable": true,
        "deprecatedAndHidden": false, "feedEnabled": true, "hasSubtypes": false,
        "isSubtype": false, "keyPrefix": "001", "labelPlural": "Accounts", "layoutable": true,
        "mergeable": true, "mruEnabled": true, "replicateable": true, "retrieveable": true,
        "searchable": true, "triggerable": true, "undeletable": true, "updateable": true,
        "urls": {}, "childRelationships": [], "recordTypeInfos": [],
        "fields": [
            field("Name", "string", "Account Name", &[]),
            field("Industry", "picklist", "Industry", &[]),
            field("Phone", "phone", "Phone", &[]),
            field("Website", "url", "Website", &[]),
            field("AnnualRevenue", "double", "Annual Revenue", &[]),
        ]
    });
    serde_json::from_value(describe_json).expect("valid Account describe")
}

/// A `Contact`-shaped describe: 15 creatable fields including a `Reference`
/// field pointing back at `Account`, matching a real org describe's shape.
fn contact_describe() -> SObjectDescribe {
    let describe_json = serde_json::json!({
        "name": "Contact", "label": "Contact", "custom": false, "queryable": true,
        "activateable": false, "createable": true, "customSetting": false, "deletable": true,
        "deprecatedAndHidden": false, "feedEnabled": true, "hasSubtypes": false,
        "isSubtype": false, "keyPrefix": "003", "labelPlural": "Contacts", "layoutable": true,
        "mergeable": true, "mruEnabled": true, "replicateable": true, "retrieveable": true,
        "searchable": true, "triggerable": true, "undeletable": true, "updateable": true,
        "urls": {}, "childRelationships": [], "recordTypeInfos": [],
        "fields": [
            field("FirstName", "string", "First Name", &[]),
            field("LastName", "string", "Last Name", &[]),
            field("AccountId", "reference", "Account ID", &["Account"]),
            field("Email", "email", "Email", &[]),
            field("Phone", "phone", "Phone", &[]),
            field("MobilePhone", "phone", "Mobile Phone", &[]),
            field("Title", "string", "Title", &[]),
            field("Department", "string", "Department", &[]),
            field("Description", "textarea", "Description", &[]),
            field("MailingCity", "string", "Mailing City", &[]),
            field("MailingState", "string", "Mailing State", &[]),
            field("Birthdate", "date", "Birthdate", &[]),
            field("LastActivityDate", "date", "Last Activity Date", &[]),
            field("HasOptedOutOfEmail", "boolean", "Email Opt Out", &[]),
            field("LeadSource", "picklist", "Lead Source", &[]),
        ]
    });
    serde_json::from_value(describe_json).expect("valid Contact describe")
}

async fn mount_endpoints(mock_server: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/services/data/v67.0/composite/graph"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "graphs": [
                { "graphId": "SeedGraph", "isSuccessful": true }
            ]
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

    let parent_describe = account_describe();
    let child_describe = contact_describe();

    let seeder = RelationalSeeder::new(&client);
    seeder
        .seed_hierarchy(&parent_describe, &child_describe, CHILD_COUNT)
        .await
        .expect("RelationalSeeder run failed");

    // Keep the result observable so the compiler can't fold the whole run away.
    println!("child_count={CHILD_COUNT}");
}
