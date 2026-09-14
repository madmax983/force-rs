//! One-shot profiling harness for `SmartIngest::execute_stream` (Bulk API 2.0).
//!
//! This is not a criterion benchmark: it is a single realistic run of the
//! public `SmartIngest` entry point, meant to be executed once under
//! `valgrind --tool=callgrind` (instruction counts) and
//! `valgrind --tool=dhat` (allocation counts) so those tools attribute cost
//! to real call stacks instead of a criterion harness loop.
//!
//! Workload: stream 20,000 `Account`-shaped records (8 fields, realistic
//! mixed-length text) through `SmartIngest` at the default batch size
//! (10,000) against an in-process mock Salesforce Bulk API 2.0 endpoint.
//! This is the same public path a caller uses for a real bulk data load;
//! record count and shape are fixed (no RNG) so repeated runs are
//! byte-for-byte deterministic, which is required for callgrind/dhat
//! comparisons to be meaningful.
//!
//! Run directly:
//! ```bash
//! cargo run --release -p force --features bulk --bench smart_ingest_profile
//! ```
//!
//! Profile (this is a `[[bench]]` target, not a `[[bin]]`: the compiled
//! executable lands under `target/release/deps/`, not `target/release/`):
//! ```bash
//! CARGO_PROFILE_RELEASE_DEBUG=true cargo build --release -p force --features bulk --bench smart_ingest_profile
//! BIN=$(find target/release/deps -maxdepth 1 -name 'smart_ingest_profile-*' -executable -not -name '*.d')
//! valgrind --tool=callgrind --callgrind-out-file=callgrind.out "$BIN"
//! callgrind_annotate callgrind.out
//!
//! valgrind --tool=dhat --dhat-out-file=dhat.out "$BIN"
//! ```
#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

use force::api::bulk::{JobOperation, SmartIngest};
use force::auth::{AccessToken, Authenticator, TokenResponse};
use force::client::builder;
use force::error::Result as ForceResult;
use futures::stream;
use serde::Serialize;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Number of records streamed through `SmartIngest` in one profiling run.
const RECORD_COUNT: usize = 20_000;

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

/// An `Account`-shaped record: 8 fields of realistic, mixed-length text,
/// matching what a real Bulk API 2.0 ingest job serializes today.
#[derive(Serialize, Clone)]
struct AccountRecord {
    #[serde(rename = "Id")]
    id: String,
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Industry")]
    industry: String,
    #[serde(rename = "Phone")]
    phone: String,
    #[serde(rename = "Website")]
    website: String,
    #[serde(rename = "BillingCity")]
    billing_city: String,
    #[serde(rename = "BillingState")]
    billing_state: String,
    #[serde(rename = "Description")]
    description: String,
}

const INDUSTRIES: [&str; 5] = ["Technology", "Finance", "Healthcare", "Retail", "Energy"];
const CITIES: [(&str, &str); 4] = [
    ("San Francisco", "CA"),
    ("Austin", "TX"),
    ("New York", "NY"),
    ("Chicago", "IL"),
];

fn build_records(count: usize) -> Vec<AccountRecord> {
    (0..count)
        .map(|i| {
            let (city, state) = CITIES[i % CITIES.len()];
            AccountRecord {
                id: format!("001xx{i:012}AAA"),
                name: format!("Account {i} Holdings Inc."),
                industry: INDUSTRIES[i % INDUSTRIES.len()].to_string(),
                phone: format!("+1-555-{:04}", i % 10_000),
                website: format!("https://account{i}.example.com"),
                billing_city: city.to_string(),
                billing_state: state.to_string(),
                description: format!(
                    "Account {i} is a long-standing customer with a multi-line \
                     description field that mirrors real Salesforce free-text data, \
                     used to exercise CSV quoting/escaping at realistic field widths."
                ),
            }
        })
        .collect()
}

async fn mount_job_lifecycle(mock_server: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/services/data/v67.0/jobs/ingest"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "PROFILE_JOB",
            "state": "Open",
            "operation": "insert",
            "object": "Account",
            "createdDate": "2024-01-01T00:00:00.000Z",
            "createdById": "005xx0000000001AAA"
        })))
        .mount(mock_server)
        .await;

    Mock::given(method("PUT"))
        .and(path("/services/data/v67.0/jobs/ingest/PROFILE_JOB/batches"))
        .respond_with(ResponseTemplate::new(201))
        .mount(mock_server)
        .await;

    Mock::given(method("PATCH"))
        .and(path("/services/data/v67.0/jobs/ingest/PROFILE_JOB"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "PROFILE_JOB",
            "state": "UploadComplete",
            "operation": "insert",
            "object": "Account",
            "createdDate": "2024-01-01T00:00:00.000Z",
            "createdById": "005xx0000000001AAA"
        })))
        .mount(mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/services/data/v67.0/jobs/ingest/PROFILE_JOB"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "PROFILE_JOB",
            "state": "JobComplete",
            "operation": "insert",
            "object": "Account",
            "createdDate": "2024-01-01T00:00:00.000Z",
            "createdById": "005xx0000000001AAA",
            "numberRecordsProcessed": RECORD_COUNT,
            "numberRecordsFailed": 0
        })))
        .mount(mock_server)
        .await;
}

#[tokio::main]
async fn main() {
    let mock_server = MockServer::start().await;
    mount_job_lifecycle(&mock_server).await;

    let auth = StaticAuthenticator {
        token: "profile-token".to_string(),
        instance_url: mock_server.uri(),
    };
    let client = builder()
        .authenticate(auth)
        .build()
        .await
        .expect("failed to build profiling client");
    let handler = client.bulk();

    let records = build_records(RECORD_COUNT);
    let record_stream = stream::iter(records);

    let result = SmartIngest::new(&handler, "Account", JobOperation::Insert)
        .execute_stream(record_stream)
        .await
        .expect("SmartIngest run failed");

    // Keep the result observable so the compiler can't fold the whole run away.
    println!(
        "jobs={} total_processed={}",
        result.job_count(),
        result.total_records_processed()
    );
}
