//! Havoc test for `DoS` vulnerability in `UsernamePassword` flow error parsing.
//!
//! This test simulates a maliciously large error response to prove memory exhaustion (OOM).

#[cfg(all(test, feature = "username_password", feature = "mock"))]
mod tests {
    use force::auth::Authenticator;
    use force::auth::UsernamePassword;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_maliciously_large_error_response() {
        let mock_server = MockServer::start().await;

        // Generate a large string to simulate a malicious response.
        // We simulate a 50MB response
        let large_body = "A".repeat(50 * 1024 * 1024);

        Mock::given(method("POST"))
            .and(path("/services/oauth2/token"))
            .respond_with(ResponseTemplate::new(400).set_body_string(large_body))
            .mount(&mock_server)
            .await;

        let auth = UsernamePassword::new(
            "client_id",
            "client_secret",
            "user@example.com",
            "password",
            "",
            format!("{}/services/oauth2/token", mock_server.uri()),
        );

        let result = auth.authenticate().await;

        let message_len = if let Err(force::error::ForceError::Authentication(
            force::error::AuthenticationError::TokenRequestFailed(msg),
        )) = result
        {
            msg.len()
        } else if let Err(force::error::ForceError::Http(force::error::HttpError::StatusError {
            message,
            ..
        })) = result
        {
            message.len()
        } else {
            panic!("Expected an error");
        };

        // Assert that the message length is strictly capped at 1MB
        assert_eq!(
            message_len,
            1024 * 1024,
            "Message length exceeded the 1MB cap! Length: {message_len}"
        );
    }
}
