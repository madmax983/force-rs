//! One-shot profiling harness for `DataExtensionsHandler::insert_rows_async`
//! (Marketing Cloud async Data Extension row insert).
//!
//! This is not a criterion benchmark: it is a single realistic run of the
//! public `client.data_extensions().insert_rows_async()` entry point, meant
//! to be executed once under `valgrind --tool=callgrind` (instruction
//! counts) and `valgrind --tool=dhat` (allocation counts) so those tools
//! attribute cost to real call stacks instead of a criterion harness loop.
//!
//! Workload: submit one `ROW_COUNT`-row batch through the public
//! `insert_rows_async` entry point against an in-process mock Marketing
//! Cloud endpoint that accepts the POST with a 202. Each row has one key
//! field (`SubscriberKey`) and `VALUE_FIELD_COUNT` value fields, matching the
//! shape of a real contact/data-extension sync batch. Row count and content
//! are fixed (no RNG) so repeated runs are byte-for-byte deterministic,
//! which is required for callgrind/dhat comparisons to be meaningful.
//!
//! `insert_rows_async` merges each row's `keys` and `values` maps into a
//! single owned `serde_json::Value::Object` before serializing the batch.
//! This harness isolates the cost of that per-row merge, which every
//! `insert_rows_async` call pays regardless of batch size.
//!
//! Run directly (`cargo run` doesn't support `--bench`, so use `cargo bench`):
//! ```bash
//! cargo bench -p force-marketingcloud --bench data_extensions_insert_rows_profile
//! ```
//!
//! Profile (this is a `[[bench]]` target, not a `[[bin]]`: the compiled
//! executable lands under `target/release/deps/`, not `target/release/`):
//! ```bash
//! CARGO_PROFILE_RELEASE_DEBUG=true cargo build --release -p force-marketingcloud --bench data_extensions_insert_rows_profile
//! BIN=$(find target/release/deps -maxdepth 1 -name 'data_extensions_insert_rows_profile-*' -executable -not -name '*.d')
//! valgrind --tool=callgrind --callgrind-out-file=callgrind.out "$BIN"
//! callgrind_annotate callgrind.out
//!
//! valgrind --tool=dhat --dhat-out-file=dhat.out "$BIN"
//! ```
#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

use force_marketingcloud::{MarketingCloudClient, Row};
use serde_json::{Map, Value, json};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Rows submitted in one `insert_rows_async` call -- a realistic batch size
/// for a nightly contact / data-extension sync job.
const ROW_COUNT: usize = 5_000;

/// Value (non-key) fields per row, matching a typical subscriber-attribute
/// data extension row.
const VALUE_FIELD_COUNT: usize = 7;

const STATUSES: [&str; 4] = ["Active", "Inactive", "Bounced", "Held"];
const TIERS: [&str; 3] = ["Bronze", "Silver", "Gold"];

/// Builds `count` rows with one key field and `VALUE_FIELD_COUNT` value
/// fields of realistic mixed-length text.
fn build_rows(count: usize) -> Vec<Row> {
    (0..count)
        .map(|i| {
            let mut keys = Map::with_capacity(1);
            keys.insert(
                "SubscriberKey".to_string(),
                Value::String(format!("sub-{i:08}")),
            );

            let mut values = Map::with_capacity(VALUE_FIELD_COUNT);
            values.insert(
                "EmailAddress".to_string(),
                Value::String(format!("contact{i}@example.com")),
            );
            values.insert("FirstName".to_string(), Value::String(format!("First{i}")));
            values.insert("LastName".to_string(), Value::String(format!("Last{i}")));
            values.insert(
                "Status".to_string(),
                Value::String(STATUSES[i % STATUSES.len()].to_string()),
            );
            values.insert(
                "LoyaltyTier".to_string(),
                Value::String(TIERS[i % TIERS.len()].to_string()),
            );
            values.insert(
                "LastActivityDate".to_string(),
                Value::String("2026-09-20T00:00:00Z".to_string()),
            );
            values.insert(
                "OptInStatus".to_string(),
                Value::String(if i % 2 == 0 { "true" } else { "false" }.to_string()),
            );

            Row::new(keys, values)
        })
        .collect()
}

async fn mount_endpoint(mock_server: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/data/v1/async/dataextensions/key:ProfileDE/rows"))
        .respond_with(ResponseTemplate::new(202).set_body_json(json!({
            "requestId": "profile-request-id",
            "resultMessages": []
        })))
        .mount(mock_server)
        .await;
}

#[tokio::main]
async fn main() {
    let mock_server = MockServer::start().await;
    mount_endpoint(&mock_server).await;

    Mock::given(method("POST"))
        .and(path("/v2/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "profile-access-token",
            "token_type": "Bearer",
            "expires_in": 7200,
            "scope": "data_extensions_read data_extensions_write",
            "soap_instance_url": format!("{}/", mock_server.uri()),
            "rest_instance_url": format!("{}/", mock_server.uri()),
        })))
        .mount(&mock_server)
        .await;

    let client: MarketingCloudClient = MarketingCloudClient::builder()
        .tenant_subdomain("profile-sub")
        .client_credentials("profile-client-id", "profile-client-secret")
        .auth_url(format!("{}/v2/token", mock_server.uri()))
        .build()
        .expect("failed to build profiling client");

    let rows = build_rows(ROW_COUNT);

    let response = client
        .data_extensions()
        .insert_rows_async("ProfileDE", &rows)
        .await
        .expect("insert_rows_async failed");

    // Keep the result observable so the compiler can't fold the whole run away.
    println!("request_id={:?} rows={}", response.request_id, rows.len());
}
