//! One-shot profiling harness for `DataArchiver::export_to_jsonl`
//! (query-to-disk JSONL export).
//!
//! This is not a criterion benchmark: it is a single realistic run of the
//! public `client.data_archiver().export_to_jsonl()` entry point (via
//! `force::data::DataArchiver::new(&client)`), meant to be executed once
//! under `valgrind --tool=callgrind` (instruction counts), `valgrind
//! --tool=dhat` (allocation counts), and `strace -c` (syscall counts, since
//! this path is I/O-bound) so those tools attribute cost to real call
//! stacks instead of a criterion harness loop.
//!
//! Workload: drain a 20,000-record SOQL query (10 pages of 2,000
//! `Account`-shaped records fetched over `nextRecordsUrl` pagination against
//! an in-process mock REST endpoint) through `query_stream` and write every
//! record to a JSONL file on disk -- the same public path a caller uses in
//! the documented "export a large object to disk" use case. Record count,
//! field count, and page count are fixed (no RNG) so repeated runs are
//! byte-for-byte deterministic, which is required for callgrind/dhat/strace
//! comparisons to be meaningful.
//!
//! Run directly (`cargo run` doesn't support `--bench`, so use `cargo bench`):
//! ```bash
//! cargo bench -p force --features data_utility --bench data_archiver_profile
//! ```
//!
//! Profile (this is a `[[bench]]` target, not a `[[bin]]`: the compiled
//! executable lands under `target/release/deps/`, not `target/release/`):
//! ```bash
//! CARGO_PROFILE_RELEASE_DEBUG=true cargo build --release -p force --features data_utility --bench data_archiver_profile
//! BIN=$(find target/release/deps -maxdepth 1 -name 'data_archiver_profile-*' -executable -not -name '*.d')
//! valgrind --tool=callgrind --callgrind-out-file=callgrind.out "$BIN"
//! callgrind_annotate callgrind.out
//!
//! valgrind --tool=dhat --dhat-out-file=dhat.out "$BIN"
//!
//! strace -f -c -o strace.out "$BIN"
//! ```
#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

use force::auth::{AccessToken, Authenticator, TokenResponse};
use force::client::builder;
use force::data::DataArchiver;
use force::error::Result as ForceResult;
use serde_json::{Value, json};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Records per page (a realistic REST query page size).
const PAGE_SIZE: usize = 2_000;
/// Number of pages fetched after the initial `query` page.
const MORE_PAGES: usize = 9;
/// Total records drained across all pages.
const TOTAL_RECORDS: usize = PAGE_SIZE * (MORE_PAGES + 1);

#[derive(Debug, Clone)]
struct StaticAuthenticator {
    token: String,
    instance_url: String,
}

/// Current time as a Salesforce-style `issued_at` (Unix ms as a string), so
/// the token this harness hands out is always freshly issued relative to
/// whenever the harness actually runs -- a fixed past timestamp would go
/// hard-expired the moment `expires_in` elapses after that fixed date,
/// forcing `TokenManager` to refresh on every request for the rest of time
/// and adding authentication noise to the profiled call stacks.
fn issued_at_now() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock before unix epoch")
        .as_millis()
        .to_string()
}

#[async_trait::async_trait]
impl Authenticator for StaticAuthenticator {
    async fn authenticate(&self) -> ForceResult<AccessToken> {
        Ok(AccessToken::from_response(TokenResponse {
            access_token: secrecy::SecretString::new(self.token.clone().into()),
            instance_url: self.instance_url.clone(),
            token_type: "Bearer".to_string(),
            issued_at: issued_at_now(),
            signature: "profile-sig".to_string(),
            expires_in: Some(7200),
            refresh_token: None,
        }))
    }

    async fn refresh(&self) -> ForceResult<AccessToken> {
        self.authenticate().await
    }
}

/// Builds one Account-shaped record.
fn record(page: usize, index: usize) -> Value {
    let n = page * PAGE_SIZE + index;
    json!({
        "attributes": {
            "type": "Account",
            "url": format!("/services/data/v67.0/sobjects/Account/001xx00000000{n}AAA"),
        },
        "Id": format!("001xx00000000{n}AAA"),
        "Name": format!("Account Number {n}"),
        "Industry": "Technology",
        "Type": "Customer - Direct",
        "Phone": format!("+1-555-01{:02}", n % 100),
        "Website": format!("https://example{n}.com"),
        "BillingCity": "Springfield",
        "BillingState": "IL",
        "AnnualRevenue": 1_000_000.0,
        "NumberOfEmployees": 250,
    })
}

fn page_records(page: usize) -> Vec<Value> {
    (0..PAGE_SIZE).map(|i| record(page, i)).collect()
}

fn page_url(page: usize) -> String {
    format!("/services/data/v67.0/query/page{page}")
}

async fn mount_endpoints(mock_server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/services/data/v67.0/query"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "totalSize": TOTAL_RECORDS,
            "done": false,
            "nextRecordsUrl": page_url(1),
            "records": page_records(0),
        })))
        .mount(mock_server)
        .await;

    for i in 1..=MORE_PAGES {
        let done = i == MORE_PAGES;
        let mut body = json!({
            "totalSize": TOTAL_RECORDS,
            "done": done,
            "records": page_records(i),
        });
        if !done {
            body["nextRecordsUrl"] = json!(page_url(i + 1));
        }
        Mock::given(method("GET"))
            .and(path(page_url(i)))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(mock_server)
            .await;
    }
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

    let archiver = DataArchiver::new(&client);
    let out_path = std::env::temp_dir().join(format!(
        "data_archiver_profile_{}.jsonl",
        std::process::id()
    ));

    let count = archiver
        .export_to_jsonl::<Value>(
            "SELECT Id, Name, Industry, Type, Phone, Website, BillingCity, BillingState, AnnualRevenue, NumberOfEmployees FROM Account",
            &out_path,
        )
        .await
        .expect("export_to_jsonl failed");

    // Keep the result observable so the compiler can't fold the whole run away.
    println!("records_written={count}");
}
