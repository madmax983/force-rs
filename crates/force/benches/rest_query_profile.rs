//! One-shot profiling harness for REST `query` / `query_more` (SOQL pagination).
//!
//! Not a criterion benchmark: a single realistic run of the public
//! `client.rest().query::<T>()` / `query_more::<T>()` entry points, meant to be
//! executed once under `valgrind --tool=callgrind` (instruction counts) and
//! `valgrind --tool=dhat` (allocation counts).
//!
//! Workload: one `query` plus 9 `query_more` calls against an in-process mock
//! REST endpoint, each page 2,000 `Account`-shaped records (10 fields plus the
//! `attributes` block), deserialized into a typed struct and then again into
//! `DynamicSObject`. Sizes are fixed (no RNG) so runs are deterministic.
//!
//! ```bash
//! CARGO_PROFILE_RELEASE_DEBUG=true cargo build --release -p force --bench rest_query_profile
//! BIN=$(find target/release/deps -maxdepth 1 -name 'rest_query_profile-*' -executable -not -name '*.d')
//! valgrind --tool=callgrind --callgrind-out-file=callgrind.out "$BIN"
//! valgrind --tool=dhat --dhat-out-file=dhat.out "$BIN"
//! ```
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::format_push_string,
    missing_docs
)]

use force::api::RestOperation;
use force::auth::{AccessToken, Authenticator, TokenResponse};
use force::client::builder;
use force::error::Result as ForceResult;
use force::types::DynamicSObject;
use serde::Deserialize;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const PAGE_SIZE: usize = 2_000;
const MORE_PAGES: usize = 9;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Account {
    #[serde(rename = "Id")]
    id: String,
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Industry")]
    industry: Option<String>,
    #[serde(rename = "Type")]
    type_: Option<String>,
    #[serde(rename = "Phone")]
    phone: Option<String>,
    #[serde(rename = "Website")]
    website: Option<String>,
    #[serde(rename = "BillingCity")]
    billing_city: Option<String>,
    #[serde(rename = "BillingState")]
    billing_state: Option<String>,
    #[serde(rename = "AnnualRevenue")]
    annual_revenue: Option<f64>,
    #[serde(rename = "NumberOfEmployees")]
    employees: Option<u32>,
}

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

fn page_body(page: usize, base: &str) -> String {
    let mut b = String::with_capacity(PAGE_SIZE * 600);
    let total = PAGE_SIZE * (MORE_PAGES + 1);
    let done = page == MORE_PAGES;
    b.push_str(&format!("{{\"totalSize\":{total},\"done\":{done},"));
    if !done {
        b.push_str(&format!(
            "\"nextRecordsUrl\":\"/services/data/v67.0/query/01gxx-{}\",",
            page + 1
        ));
    }
    b.push_str("\"records\":[");
    for i in 0..PAGE_SIZE {
        if i > 0 {
            b.push(',');
        }
        let n = page * PAGE_SIZE + i;
        b.push_str(&format!(
            "{{\"attributes\":{{\"type\":\"Account\",\"url\":\"/services/data/v67.0/sobjects/Account/001xx00000{n:06}AAA\"}},\"Id\":\"001xx00000{n:06}AAA\",\"Name\":\"Account Number {n}\",\"Industry\":\"Technology\",\"Type\":\"Customer - Direct\",\"Phone\":\"+1-555-01{}\",\"Website\":\"https://example{n}.com\",\"BillingCity\":\"Springfield\",\"BillingState\":\"IL\",\"AnnualRevenue\":1000000.0,\"NumberOfEmployees\":250}}",
            n % 100
        ));
    }
    b.push_str("]}");
    let _ = base;
    b
}

async fn mount(server: &MockServer) {
    let json = |s: String| {
        ResponseTemplate::new(200)
            .insert_header("Content-Type", "application/json")
            .set_body_string(s)
    };
    Mock::given(method("GET"))
        .and(path("/services/data/v67.0/query"))
        .and(query_param("q", "SELECT Id FROM Account"))
        .respond_with(json(page_body(0, "")))
        .mount(server)
        .await;
    for i in 1..=MORE_PAGES {
        Mock::given(method("GET"))
            .and(path(format!("/services/data/v67.0/query/01gxx-{i}")))
            .respond_with(json(page_body(i, "")))
            .mount(server)
            .await;
    }
}

#[tokio::main]
async fn main() {
    let server = MockServer::start().await;
    mount(&server).await;
    let auth = StaticAuthenticator {
        token: "profile-token".to_string(),
        instance_url: server.uri(),
    };
    let client = builder().authenticate(auth).build().await.expect("client");
    let rest = client.rest();

    let mut total = 0usize;
    let mut r = rest
        .query::<Account>("SELECT Id FROM Account")
        .await
        .expect("q");
    total += r.records.len();
    while let Some(next) = r.next_records_url.clone() {
        r = rest.query_more::<Account>(&next).await.expect("more");
        total += r.records.len();
    }

    let mut r = rest
        .query::<DynamicSObject>("SELECT Id FROM Account")
        .await
        .expect("q");
    total += r.records.len();
    while let Some(next) = r.next_records_url.clone() {
        r = rest
            .query_more::<DynamicSObject>(&next)
            .await
            .expect("more");
        total += r.records.len();
    }
    println!("total_records={total}");
}
