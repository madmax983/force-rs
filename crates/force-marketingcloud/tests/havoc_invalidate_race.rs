#![allow(clippy::unwrap_used)]
#![allow(missing_docs)]
use force_marketingcloud::MarketingCloudClient;
use force_marketingcloud::Recipient;
use force_marketingcloud::SendEmailRequest;
use wiremock::matchers::header;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn test_stale_resurrection() {
    let server = MockServer::start().await;

    // A slow auth endpoint (taking 100ms)
    Mock::given(method("POST"))
        .and(path("/v2/token"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({
                    "access_token": "stale_token",
                    "expires_in": 3600,
                    "token_type": "Bearer",
                    "rest_instance_url": server.uri()
                }))
                .set_delay(std::time::Duration::from_millis(100)),
        )
        .mount(&server)
        .await;

    // Mock API endpoint to capture the authorization header
    Mock::given(method("POST"))
        .and(path("/messaging/v1/email/messages/MSG-KEY"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "requestId": "123",
            "errorcode": 0,
            "responses": []
        })))
        .mount(&server)
        .await;

    // Create client
    let client = MarketingCloudClient::builder()
        .tenant_subdomain("dummy")
        .auth_url(format!("{}/v2/token", server.uri()))
        .client_credentials("test_id", "test_secret")
        .build()
        .unwrap();

    // In force_marketingcloud, the base url is extracted from the token's rest_instance_url!
    // So we don't need a base_url_override for the client.

    let req = SendEmailRequest::new("transactional_welcome", Recipient::new("1", "x@x.com"));

    // Task 1: Start an API call which triggers token fetch (takes 100ms)
    let c1 = client.clone();
    let r1 = req.clone();
    let t1 = tokio::spawn(async move {
        // This will block on auth
        let _ = c1.transactional().send_email("MSG-KEY", &r1).await;
    });

    // Let Task 1 get past the cache check and start "authenticating"
    tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;

    // Task 2: Some other process calls invalidate
    client.invalidate_token(None).await;

    // Wait for Task 1 to finish auth and insert the token
    t1.await.unwrap();

    // Now, change the mock to return a fast, new token
    server.reset().await;

    // We expect a new token to be fetched, so we mock the auth endpoint again
    Mock::given(method("POST"))
        .and(path("/v2/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "new_fresh_token",
            "expires_in": 3600,
            "token_type": "Bearer",
            "rest_instance_url": server.uri()
        })))
        // Expect this to be called once, because the cache should have been invalidated
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/messaging/v1/email/messages/MSG-KEY-2"))
        // We assert that the token sent was indeed the new one
        .and(header("Authorization", "Bearer new_fresh_token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "requestId": "123",
            "errorcode": 0,
            "responses": []
        })))
        .mount(&server)
        .await;

    // Task 3: Make an API call. If the invalidate was successful, it should fetch a new token.
    // If the cache was resurrected by the slow auth, it will use "stale_token" (and we won't hit the new Mock).
    let _res = client.transactional().send_email("MSG-KEY-2", &req).await;

    // If _res is Err because the token was "stale_token" and didn't match the new mock, it proves the race!
}
