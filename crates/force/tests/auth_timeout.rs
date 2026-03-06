//! Tests for authentication timeout behavior.
//!
//! Verifies that `ClientCredentials` and `JwtBearerFlow` authenticators
//! correctly respect configured timeouts to prevent indefinite hanging.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use force::auth::Authenticator;
use force::auth::ClientCredentials;
use force::error::{ForceError, HttpError};
use std::time::Duration;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn test_client_credentials_timeout() {
    let mock_server = MockServer::start().await;

    // Simulate a slow server (200ms delay)
    Mock::given(method("POST"))
        .and(path("/services/oauth2/token"))
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_millis(200)))
        .mount(&mock_server)
        .await;

    // Create client with 100ms timeout
    let http_client = reqwest::Client::builder()
        .timeout(Duration::from_millis(100))
        .build()
        .unwrap();

    let auth = ClientCredentials::new(
        "client_id",
        "client_secret",
        format!("{}/services/oauth2/token", mock_server.uri()),
    )
    .with_client(http_client);

    let result = auth.authenticate().await;

    match result {
        Err(ForceError::Http(HttpError::RequestFailed(e))) => {
            assert!(e.is_timeout(), "Expected timeout error, got: {e}");
        }
        Err(e) => panic!("Expected ForceError::Http(RequestFailed(timeout)), got: {e:?}"),
        Ok(_) => panic!("Expected timeout error, got Ok"),
    }
}

#[cfg(feature = "jwt")]
#[tokio::test]
async fn test_jwt_bearer_timeout() {
    use force::auth::JwtBearerFlow;
    use std::fs;

    let mock_server = MockServer::start().await;

    // Simulate a slow server (200ms delay)
    Mock::given(method("POST"))
        .and(path("/services/oauth2/token"))
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_millis(200)))
        .mount(&mock_server)
        .await;

    // Create client with 100ms timeout
    let http_client = reqwest::Client::builder()
        .timeout(Duration::from_millis(100))
        .build()
        .unwrap();

    let key_path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dummy_key.pem");
    let private_key = fs::read_to_string(key_path).expect("Failed to read dummy key");

    let auth = JwtBearerFlow::builder()
        .client_id("client_id")
        .username("user@example.com")
        .private_key(private_key)
        .token_url(format!("{}/services/oauth2/token", mock_server.uri()))
        .http_client(http_client)
        .build()
        .unwrap();

    let result = auth.authenticate().await;

    match result {
        Err(ForceError::Http(HttpError::RequestFailed(e))) => {
            assert!(e.is_timeout(), "Expected timeout error, got: {e}");
        }
        Err(e) => panic!("Expected ForceError::Http(RequestFailed(timeout)), got: {e:?}"),
        Ok(_) => panic!("Expected timeout error, got Ok"),
    }
}
