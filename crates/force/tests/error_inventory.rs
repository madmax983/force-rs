//! Onramp error-message inventory (first-run failure modes).
//!
//! Each row triggers one failure a newcomer commonly hits and scores the
//! rendered `Display` text on three booleans:
//!   * names the cause   - contains the Salesforce/OAuth error code or reason
//!   * states the fix    - contains an imperative/hint ("check", "verify", "use", "enable", ...)
//!   * points to the doc - contains a docs path/URL
//!
//! Run: `cargo test -p force --test error_inventory -- --nocapture`
//! The final line prints the tally; `ONRAMP_INVENTORY_MIN_FULL` (default 0)
//! sets the number of fully-passing rows required, so the baseline is
//! recorded as a number and can be ratcheted up as messages improve.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::many_single_char_names)]

use force::api::RestOperation;
use force::auth::ClientCredentials;
use force::client::builder;
use serde_json::json;
use wiremock::matchers::{method, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};

struct Row {
    name: &'static str,
    msg: String,
    cause: &'static str,
}

fn score(msg: &str, cause: &str) -> (bool, bool, bool) {
    let l = msg.to_lowercase();
    let names = l.contains(&cause.to_lowercase());
    let fix = [
        "check ", "verify ", "ensure ", "enable ", "use ", "set ", "try ", "run ", "grant ",
    ]
    .iter()
    .any(|k| l.contains(k));
    let doc =
        l.contains("docs/") || l.contains("http://") || l.contains("https://") || l.contains(".md");
    (names, fix, doc)
}

async fn token_server(token_status: u16, token_body: serde_json::Value) -> MockServer {
    let s = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path_regex("/services/oauth2/token"))
        .respond_with(ResponseTemplate::new(token_status).set_body_json(token_body))
        .mount(&s)
        .await;
    s
}

fn ok_token(uri: &str) -> serde_json::Value {
    json!({"access_token":"tok","instance_url":uri,"token_type":"Bearer","issued_at":"1","signature":"s"})
}

#[tokio::test]
async fn error_message_inventory() {
    let mut rows: Vec<Row> = Vec::new();

    // 1. Wrong consumer key/secret.
    let s = token_server(
        400,
        json!({"error":"invalid_client_id","error_description":"client identifier invalid"}),
    )
    .await;
    let auth = ClientCredentials::new("bad", "bad", format!("{}/services/oauth2/token", s.uri()));
    let client = builder().authenticate(auth).build().await.unwrap();
    let e = client
        .rest()
        .query::<serde_json::Value>("SELECT Id FROM Account")
        .await
        .err()
        .unwrap();
    rows.push(Row {
        name: "invalid consumer key",
        msg: e.to_string(),
        cause: "client identifier invalid",
    });

    // 2. Client Credentials flow not enabled / no run-as user.
    let s = token_server(
        400,
        json!({"error":"invalid_grant","error_description":"no client credentials user enabled"}),
    )
    .await;
    let auth = ClientCredentials::new("id", "sec", format!("{}/services/oauth2/token", s.uri()));
    let client = builder().authenticate(auth).build().await.unwrap();
    let e = client
        .rest()
        .query::<serde_json::Value>("SELECT Id FROM Account")
        .await
        .err()
        .unwrap();
    rows.push(Row {
        name: "client-credentials user not set",
        msg: e.to_string(),
        cause: "client credentials user",
    });

    // 3-5. API-level failures after a good token.
    for (name, status, body, cause) in [
        (
            "REST API disabled for org",
            403,
            json!([{"message":"The REST API is not enabled for this Organization.","errorCode":"API_DISABLED_FOR_ORG"}]),
            "API_DISABLED_FOR_ORG",
        ),
        (
            "malformed SOQL",
            400,
            json!([{"message":"unexpected token: 'FORM'","errorCode":"MALFORMED_QUERY"}]),
            "MALFORMED_QUERY",
        ),
        (
            "unknown sObject",
            404,
            json!([{"message":"The requested resource does not exist","errorCode":"NOT_FOUND"}]),
            "NOT_FOUND",
        ),
    ] {
        let s = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path_regex("/services/oauth2/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(ok_token(&s.uri())))
            .mount(&s)
            .await;
        Mock::given(method("GET"))
            .and(path_regex("/services/data/.*"))
            .respond_with(ResponseTemplate::new(status).set_body_json(body))
            .mount(&s)
            .await;
        let auth =
            ClientCredentials::new("id", "sec", format!("{}/services/oauth2/token", s.uri()));
        let client = builder().authenticate(auth).build().await.unwrap();
        let e = client
            .rest()
            .query::<serde_json::Value>("SELECT Id FORM Account")
            .await
            .err()
            .unwrap();
        rows.push(Row {
            name,
            msg: e.to_string(),
            cause,
        });
    }

    println!(
        "\n| failure mode | names cause | states fix | points to doc | message |\n|---|---|---|---|---|"
    );
    let mut full = 0;
    for r in &rows {
        let (a, b, c) = score(&r.msg, r.cause);
        if a && b && c {
            full += 1;
        }
        let m = |x: bool| if x { "✓" } else { "✗" };
        println!(
            "| {} | {} | {} | {} | `{}` |",
            r.name,
            m(a),
            m(b),
            m(c),
            r.msg
        );
    }
    println!("INVENTORY full={full}/{}", rows.len());
    let min: usize = std::env::var("ONRAMP_INVENTORY_MIN_FULL")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5);
    assert!(
        full >= min,
        "only {full} fully-informative rows, need {min}"
    );
}
