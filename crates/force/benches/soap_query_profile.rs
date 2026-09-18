//! One-shot profiling harness for `SoapHandler::query` / `query_more`
//! (SOAP Partner API record parsing).
//!
//! This is not a criterion benchmark: it is a single realistic run of the
//! public `client.soap().query()` / `query_more()` entry points, meant to be
//! executed once under `valgrind --tool=callgrind` (instruction counts) and
//! `valgrind --tool=dhat` (allocation counts) so those tools attribute cost
//! to real call stacks instead of a criterion harness loop.
//!
//! Workload: run one `query()` call followed by 9 `query_more()` calls against
//! an in-process mock SOAP endpoint, each returning a 2,000-record
//! `Account`-shaped page (10 `sf:`-namespaced string fields plus the `type`
//! element and the WSDL's duplicated leading `Id` element), for 20,000 total
//! records parsed through the untyped DOM parser
//! (`force::api::soap::parse::parse_record`, reached only via the public
//! `query`/`query_more` handlers). This is the same public path a caller
//! loops through to drain a large SOQL query via the classic SOAP Partner
//! API. Record count, field count, and page count are fixed (no RNG) so
//! repeated runs are byte-for-byte deterministic, which is required for
//! callgrind/dhat comparisons to be meaningful.
//!
//! Run directly (`cargo run` doesn't support `--bench`, so use `cargo bench`):
//! ```bash
//! cargo bench -p force --features soap --bench soap_query_profile
//! ```
//!
//! Profile (this is a `[[bench]]` target, not a `[[bin]]`: the compiled
//! executable lands under `target/release/deps/`, not `target/release/`):
//! ```bash
//! CARGO_PROFILE_RELEASE_DEBUG=true cargo build --release -p force --features soap --bench soap_query_profile
//! BIN=$(find target/release/deps -maxdepth 1 -name 'soap_query_profile-*' -executable -not -name '*.d')
//! valgrind --tool=callgrind --callgrind-out-file=callgrind.out "$BIN"
//! callgrind_annotate callgrind.out
//!
//! valgrind --tool=dhat --dhat-out-file=dhat.out "$BIN"
//! ```
#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

use force::auth::{AccessToken, Authenticator, TokenResponse};
use force::client::builder;
use force::error::Result as ForceResult;
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Records per page (a realistic SOAP query page size).
const PAGE_SIZE: usize = 2_000;
/// Number of `query_more` pages fetched after the initial `query` page.
const MORE_PAGES: usize = 9;

const NS: &str = concat!(
    r#" xmlns:soapenv="http://schemas.xmlsoap.org/soap/envelope/""#,
    r#" xmlns="urn:partner.soap.sforce.com""#,
    r#" xmlns:sf="urn:sobject.partner.soap.sforce.com""#,
    r#" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance""#,
);

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

/// Builds one `<records>` element: the WSDL's duplicated leading `Id`
/// (a documented quirk `parse_record` de-duplicates) plus `type` and 10
/// `Account`-shaped string fields.
fn record_xml(out: &mut String, index: usize) {
    out.push_str("<records>");
    out.push_str("<sf:type>Account</sf:type>");
    out.push_str("<sf:Id>001xx00000000");
    out.push_str(&index.to_string());
    out.push_str("AAA</sf:Id><sf:Id>001xx00000000");
    out.push_str(&index.to_string());
    out.push_str("AAA</sf:Id>");
    out.push_str("<sf:Name>Account Number ");
    out.push_str(&index.to_string());
    out.push_str("</sf:Name>");
    out.push_str("<sf:Industry>Technology</sf:Industry>");
    out.push_str("<sf:Type>Customer - Direct</sf:Type>");
    out.push_str("<sf:Phone>+1-555-01");
    out.push_str(&(index % 100).to_string());
    out.push_str("</sf:Phone>");
    out.push_str("<sf:Website>https://example");
    out.push_str(&index.to_string());
    out.push_str(".com</sf:Website>");
    out.push_str("<sf:BillingCity>Springfield</sf:BillingCity>");
    out.push_str("<sf:BillingState>IL</sf:BillingState>");
    out.push_str("<sf:AnnualRevenue>1000000.0</sf:AnnualRevenue>");
    out.push_str("<sf:NumberOfEmployees>250</sf:NumberOfEmployees>");
    out.push_str("</records>");
}

/// Builds a `queryResponse`/`queryMoreResponse` body for one page.
fn page_body(response_el: &str, done: bool, locator: Option<&str>) -> String {
    let mut body = String::with_capacity(PAGE_SIZE * 512 + 512);
    body.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?><soapenv:Envelope");
    body.push_str(NS);
    body.push_str("><soapenv:Body><");
    body.push_str(response_el);
    body.push_str("><result xsi:type=\"QueryResult\"><done>");
    body.push_str(if done { "true" } else { "false" });
    body.push_str("</done><queryLocator");
    match locator {
        Some(loc) => {
            body.push('>');
            body.push_str(loc);
            body.push_str("</queryLocator>");
        }
        None => body.push_str(" xsi:nil=\"true\"/>"),
    }
    for i in 0..PAGE_SIZE {
        record_xml(&mut body, i);
    }
    body.push_str("<size>");
    body.push_str(&PAGE_SIZE.to_string());
    body.push_str("</size></result></");
    body.push_str(response_el);
    body.push_str("></soapenv:Body></soapenv:Envelope>");
    body
}

async fn mount_endpoints(mock_server: &MockServer) {
    let locator = |i: usize| format!("loc-{i}");

    // Initial `query()` call.
    Mock::given(method("POST"))
        .and(path("/services/Soap/u/67.0"))
        .and(body_string_contains("<urn:query>"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("Content-Type", "text/xml; charset=utf-8")
                .set_body_string(page_body("queryResponse", false, Some(&locator(1)))),
        )
        .mount(mock_server)
        .await;

    // `query_more()` pages 1..MORE_PAGES, keyed on the locator each request carries.
    for i in 1..=MORE_PAGES {
        let done = i == MORE_PAGES;
        let next_locator = if done { None } else { Some(locator(i + 1)) };
        Mock::given(method("POST"))
            .and(path("/services/Soap/u/67.0"))
            .and(body_string_contains(format!(
                "<urn:queryLocator>{}",
                locator(i)
            )))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("Content-Type", "text/xml; charset=utf-8")
                    .set_body_string(page_body(
                        "queryMoreResponse",
                        done,
                        next_locator.as_deref(),
                    )),
            )
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

    let soap = client.soap();
    let mut result = soap
        .query("SELECT Id, Name, Industry, Type, Phone, Website, BillingCity, BillingState, AnnualRevenue, NumberOfEmployees FROM Account")
        .await
        .expect("initial query failed");
    let mut total_records = result.records.len();

    while !result.done {
        let locator = result
            .query_locator
            .clone()
            .expect("non-done result must carry a query locator");
        result = soap.query_more(&locator).await.expect("query_more failed");
        total_records += result.records.len();
    }

    // Keep the result observable so the compiler can't fold the whole run away.
    println!("total_records={total_records}");
}
