use force::auth::ClientCredentials;
use force::auth::authenticator::Authenticator;
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

    // Should fail with timeout
    assert!(result.is_err());
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

    let key_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/dummy_key.pem");
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

    // Should fail with timeout
    assert!(result.is_err());
}
