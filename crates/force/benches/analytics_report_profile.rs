//! One-shot profiling harness for `AnalyticsHandler::run_report`
//! (Reports & Dashboards REST API result parsing).
//!
//! This is not a criterion benchmark: it is a single realistic run of the
//! public `client.analytics().run_report()` entry point, meant to be executed
//! once under `valgrind --tool=callgrind` (instruction counts) and
//! `valgrind --tool=dhat` (allocation counts) so those tools attribute cost
//! to real call stacks instead of a criterion harness loop.
//!
//! Workload: run `run_report(id, include_details = true)` `ITER_COUNT` times
//! against an in-process mock Analytics endpoint, each response a tabular
//! report with `ROW_COUNT` detail rows of `COLUMN_COUNT` columns (mixed
//! string/number cell values) under a single `"T!T"` fact-map entry, plus
//! populated `reportMetadata`/`reportExtendedMetadata` (both of which carry
//! `#[serde(flatten)]` maps). This is the same public path a caller uses to
//! pull a large tabular report export. Record count, column count, and
//! content are fixed (no RNG) so repeated runs are byte-for-byte
//! deterministic, which is required for callgrind/dhat comparisons to be
//! meaningful.
//!
//! Run directly (`cargo run` doesn't support `--bench`, so use `cargo bench`):
//! ```bash
//! cargo bench -p force --features analytics --bench analytics_report_profile
//! ```
//!
//! Profile (this is a `[[bench]]` target, not a `[[bin]]`: the compiled
//! executable lands under `target/release/deps/`, not `target/release/`):
//! ```bash
//! CARGO_PROFILE_RELEASE_DEBUG=true cargo build --release -p force --features analytics --bench analytics_report_profile
//! BIN=$(find target/release/deps -maxdepth 1 -name 'analytics_report_profile-*' -executable -not -name '*.d')
//! valgrind --tool=callgrind --callgrind-out-file=callgrind.out "$BIN"
//! callgrind_annotate callgrind.out
//!
//! valgrind --tool=dhat --dhat-out-file=dhat.out "$BIN"
//! ```
#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

use force::auth::{AccessToken, Authenticator, TokenResponse};
use force::client::builder;
use force::error::Result as ForceResult;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Detail rows in the fixture report (a realistic large tabular export).
const ROW_COUNT: usize = 20_000;
/// Number of `run_report` calls in the workload (repeated pulls, e.g. a
/// scheduled export job iterating several reports).
const ITER_COUNT: usize = 5;

const COLUMNS: [&str; 8] = [
    "Id",
    "Name",
    "Industry",
    "Type",
    "Phone",
    "BillingCity",
    "BillingState",
    "AnnualRevenue",
];

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

/// Appends one detail row's `dataCells` array for row `index`.
fn push_row(out: &mut String, index: usize) {
    out.push_str(r#"{"dataCells":["#);
    for (col_idx, col) in COLUMNS.iter().enumerate() {
        if col_idx > 0 {
            out.push(',');
        }
        match *col {
            "Id" => {
                out.push_str(r#"{"label":"001xx00000000"#);
                out.push_str(&index.to_string());
                out.push_str(r#"AAA","value":"001xx00000000"#);
                out.push_str(&index.to_string());
                out.push_str(r#"AAA"}"#);
            }
            "Name" => {
                out.push_str(r#"{"label":"Account Number "#);
                out.push_str(&index.to_string());
                out.push_str(r#"","value":"Account Number "#);
                out.push_str(&index.to_string());
                out.push_str(r#""}"#);
            }
            "Industry" => out.push_str(r#"{"label":"Technology","value":"Technology"}"#),
            "Type" => out.push_str(r#"{"label":"Customer - Direct","value":"Customer - Direct"}"#),
            "Phone" => {
                out.push_str(r#"{"label":"+1-555-01"#);
                out.push_str(&(index % 100).to_string());
                out.push_str(r#"","value":"+1-555-01"#);
                out.push_str(&(index % 100).to_string());
                out.push_str(r#""}"#);
            }
            "BillingCity" => out.push_str(r#"{"label":"Springfield","value":"Springfield"}"#),
            "BillingState" => out.push_str(r#"{"label":"IL","value":"IL"}"#),
            "AnnualRevenue" => out.push_str(r#"{"label":"$1,000,000.00","value":1000000.0}"#),
            _ => unreachable!(),
        }
    }
    out.push_str("]}");
}

/// Builds a full `ReportResults` JSON body with `ROW_COUNT` detail rows.
fn build_report_body(report_id: &str) -> String {
    let mut out = String::with_capacity(ROW_COUNT * 320 + 4096);
    out.push_str(r#"{"attributes":{"type":"Report","reportId":""#);
    out.push_str(report_id);
    out.push_str(r#"","reportName":"Accounts By Industry","describeUrl":"/services/data/v67.0/analytics/reports/"#);
    out.push_str(report_id);
    out.push_str(r#"/describe","instancesUrl":"/services/data/v67.0/analytics/reports/"#);
    out.push_str(report_id);
    out.push_str(r#"/instances"},"allData":true,"factMap":{"T!T":{"aggregates":[{"label":""#);
    out.push_str(&ROW_COUNT.to_string());
    out.push_str(r#"","value":"#);
    out.push_str(&ROW_COUNT.to_string());
    out.push_str(r#"}],"rows":["#);
    for i in 0..ROW_COUNT {
        if i > 0 {
            out.push(',');
        }
        push_row(&mut out, i);
    }
    out.push_str(r#"]}},"groupingsAcross":{"groupings":[]},"groupingsDown":{"groupings":[]},"#);
    out.push_str(r#""hasDetailRows":true,"reportMetadata":{"id":""#);
    out.push_str(report_id);
    out.push_str(r#"","name":"Accounts By Industry","developerName":"Accounts_By_Industry","reportType":{"type":"Account","label":"Accounts"},"reportFormat":"TABULAR","reportFilters":[{"column":"Account.Industry","operator":"notEqual","value":""}],"detailColumns":["#);
    for (i, col) in COLUMNS.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push('"');
        out.push_str("Account.");
        out.push_str(col);
        out.push('"');
    }
    out.push_str(r#"],"aggregates":["RowCount"],"groupingsDown":[],"groupingsAcross":[],"scope":"organization","currency":"USD","hasDetailRows":true,"hasRecordCount":true,"showGrandTotal":true,"showSubtotals":true},"reportExtendedMetadata":{"detailColumnInfo":{"#);
    for (i, col) in COLUMNS.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push('"');
        out.push_str("Account.");
        out.push_str(col);
        out.push_str(r#"":{"label":""#);
        out.push_str(col);
        out.push_str(r#"","dataType":"stringtype"}"#);
    }
    out.push_str(r#"},"aggregateColumnInfo":{"RowCount":{"label":"Record Count","dataType":"int"}},"groupingColumnInfo":{}},"hasExceededTabularRowLimit":false}"#);
    out
}

async fn mount_endpoint(mock_server: &MockServer, report_id: &str, body: &str) {
    Mock::given(method("GET"))
        .and(path(format!(
            "/services/data/v67.0/analytics/reports/{report_id}"
        )))
        .and(query_param("includeDetails", "true"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("Content-Type", "application/json")
                .set_body_string(body),
        )
        .mount(mock_server)
        .await;
}

#[tokio::main]
async fn main() {
    let report_id = "00O3000000B5Yn2";
    let body = build_report_body(report_id);

    let mock_server = MockServer::start().await;
    mount_endpoint(&mock_server, report_id, &body).await;

    let auth = StaticAuthenticator {
        token: "profile-token".to_string(),
        instance_url: mock_server.uri(),
    };
    let client = builder()
        .authenticate(auth)
        .build()
        .await
        .expect("failed to build profiling client");

    let analytics = client.analytics();
    let mut total_rows = 0usize;
    for _ in 0..ITER_COUNT {
        let results = analytics
            .run_report(report_id, true)
            .await
            .expect("run_report failed");
        total_rows += results.fact_map["T!T"].rows.len();
    }

    // Keep the result observable so the compiler can't fold the whole run away.
    println!("total_rows={total_rows}");
}
