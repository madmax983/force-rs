//! One-shot profiling harness for the Composite mass-update path
//! (`QueryBatch::run`: SOQL query → paginate → batch PATCH in groups of 25).
//!
//! Not a criterion benchmark: a single realistic run through the public
//! `QueryBatch` entry point against an in-process mock server, meant to be
//! executed once under `valgrind --tool=callgrind` / `--tool=dhat`.
//!
//! Workload: 5 pages x 1,000 `Account`-shaped records (10 fields), each record
//! turned into a PATCH, flushed as 200 composite batches of 25 subrequests.
//! Sizes are fixed (no RNG) so runs are deterministic.
//!
//! ```bash
//! CARGO_PROFILE_RELEASE_DEBUG=true cargo build --release -p force --bench composite_query_batch_profile
//! BIN=$(find target/release/deps -maxdepth 1 -name 'composite_query_batch_profile-*' -executable -not -name '*.d')
//! valgrind --tool=callgrind --callgrind-out-file=callgrind.out "$BIN"
//! valgrind --tool=dhat --dhat-out-file=dhat.out "$BIN"
//! ```
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::format_push_string,
    missing_docs
)]

use force::api::composite::{BatchOp, QueryBatch};
use force::auth::{AccessToken, Authenticator, TokenResponse};
use force::client::builder;
use force::error::Result as ForceResult;
use serde::Deserialize;
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

const PAGE_SIZE: usize = 1_000;
const PAGES: usize = 5;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Account {
    #[serde(rename = "Id")]
    id: String,
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Industry")]
    industry: Option<String>,
    #[serde(rename = "Phone")]
    phone: Option<String>,
}

#[derive(Debug, Clone)]
struct StaticAuthenticator {
    instance_url: String,
}

#[async_trait::async_trait]
impl Authenticator for StaticAuthenticator {
    async fn authenticate(&self) -> ForceResult<AccessToken> {
        Ok(AccessToken::from_response(TokenResponse {
            access_token: secrecy::SecretString::new("profile-token".to_string().into()),
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

fn page_body(page: usize) -> String {
    let mut out = String::with_capacity(PAGE_SIZE * 420);
    out.push_str(&format!(
        r#"{{"totalSize":{},"done":{},"#,
        PAGE_SIZE * PAGES,
        page + 1 == PAGES
    ));
    if page + 1 < PAGES {
        out.push_str(&format!(
            r#""nextRecordsUrl":"/services/data/v67.0/query/01gPAGE{:04}","#,
            page + 1
        ));
    }
    out.push_str(r#""records":["#);
    for i in 0..PAGE_SIZE {
        let n = page * PAGE_SIZE + i;
        if i > 0 {
            out.push(',');
        }
        out.push_str(&format!(
            r#"{{"attributes":{{"type":"Account","url":"/services/data/v67.0/sobjects/Account/001xx{n:010}AAA"}},"Id":"001xx{n:010}AAA","Name":"Account Number {n}","Industry":"Technology","Phone":"+1-555-01{:02}","Type":"Customer - Direct","BillingCity":"Springfield","BillingState":"IL","AnnualRevenue":1000000.0,"Website":"https://example{n}.com"}}"#,
            n % 100
        ));
    }
    out.push_str("]}");
    out
}

struct BatchResponder;

impl Respond for BatchResponder {
    fn respond(&self, req: &Request) -> ResponseTemplate {
        let v: serde_json::Value = serde_json::from_slice(&req.body).unwrap();
        let n = v["batchRequests"].as_array().unwrap().len();
        let results: Vec<_> = (0..n)
            .map(|_| json!({"statusCode": 204, "result": null}))
            .collect();
        ResponseTemplate::new(200).set_body_json(json!({"hasErrors": false, "results": results}))
    }
}

#[tokio::main]
async fn main() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/services/data/v67.0/query"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("Content-Type", "application/json")
                .set_body_string(page_body(0)),
        )
        .mount(&server)
        .await;
    for p in 1..PAGES {
        Mock::given(method("GET"))
            .and(path(format!("/services/data/v67.0/query/01gPAGE{p:04}")))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("Content-Type", "application/json")
                    .set_body_string(page_body(p)),
            )
            .mount(&server)
            .await;
    }
    Mock::given(method("POST"))
        .and(path("/services/data/v67.0/composite/batch"))
        .respond_with(BatchResponder)
        .mount(&server)
        .await;

    let client = builder()
        .authenticate(StaticAuthenticator {
            instance_url: server.uri(),
        })
        .build()
        .await
        .expect("failed to build profiling client");

    let stats = QueryBatch::new(&client, "SELECT Id, Name, Industry, Phone FROM Account")
        .run(|a: Account| {
            Some(BatchOp::Update(
                "Account".to_string(),
                a.id,
                json!({"Industry": "Technology", "Description": format!("Reviewed {}", a.name)}),
            ))
        })
        .await
        .expect("query batch failed");

    println!(
        "processed={} ok={} failed={}",
        stats.records_processed, stats.ops_succeeded, stats.ops_failed
    );
}
