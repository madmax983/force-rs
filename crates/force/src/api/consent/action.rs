//! Consent action read endpoints.
//!
//! Provides consent status checks across multiple records and actions.

use super::ConsentHandler;
use super::types::ConsentResponse;
use crate::error::{HttpError, Result};

impl<A: crate::auth::Authenticator> ConsentHandler<A> {
    /// Reads consent status for a single action across multiple records.
    ///
    /// Queries the consent API to check whether the specified records have
    /// consented to the given action (e.g., `"email"`, `"track"`,
    /// `"shouldForget"`).
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let result = client.consent()
    ///     .read_consent("email", &["001xx000003GYk1", "001xx000003GYk2"])
    ///     .await?;
    ///
    /// for (id, record) in &result {
    ///     if record.proceed.get("email") == Some(&ConsentValue::Yes) {
    ///         println!("{id}: email consent granted");
    ///     }
    /// }
    /// ```
    pub async fn read_consent(&self, action: &str, ids: &[&str]) -> Result<ConsentResponse> {
        let url = self
            .resolve_consent_url(&format!("action/{action}"))
            .await?;
        // ⚡ Bolt: Construct comma-separated string dynamically to avoid intermediate Vec allocation from `.join()`
        let mut ids_param = String::with_capacity(ids.iter().map(|s| s.len() + 1).sum());
        for (i, id) in ids.iter().enumerate() {
            if i > 0 {
                ids_param.push(',');
            }
            ids_param.push_str(id);
        }
        let request = self
            .inner
            .get(&url)
            .query(&[("ids", ids_param.as_str())])
            .build()
            .map_err(HttpError::from)?;
        self.inner
            .send_request_and_decode(request, &format!("Consent read action/{action} failed"))
            .await
    }

    /// Reads consent status for multiple actions across multiple records.
    ///
    /// Queries the consent multiaction API to check consent for several
    /// actions at once (e.g., `["email", "track"]`).
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let result = client.consent()
    ///     .read_consent_multi(
    ///         &["email", "track", "shouldForget"],
    ///         &["001xx000003GYk1"],
    ///     )
    ///     .await?;
    ///
    /// for (id, record) in &result {
    ///     println!("{id}: email={:?}, track={:?}",
    ///         record.proceed.get("email"),
    ///         record.proceed.get("track"),
    ///     );
    /// }
    /// ```
    pub async fn read_consent_multi(
        &self,
        actions: &[&str],
        ids: &[&str],
    ) -> Result<ConsentResponse> {
        let url = self.resolve_consent_url("multiaction").await?;
        // ⚡ Bolt: Construct comma-separated string dynamically to avoid intermediate Vec allocation from `.join()`
        let mut actions_param = String::with_capacity(actions.iter().map(|s| s.len() + 1).sum());
        for (i, action) in actions.iter().enumerate() {
            if i > 0 {
                actions_param.push(',');
            }
            actions_param.push_str(action);
        }
        // ⚡ Bolt: Construct comma-separated string dynamically to avoid intermediate Vec allocation from `.join()`
        let mut ids_param = String::with_capacity(ids.iter().map(|s| s.len() + 1).sum());
        for (i, id) in ids.iter().enumerate() {
            if i > 0 {
                ids_param.push(',');
            }
            ids_param.push_str(id);
        }
        let request = self
            .inner
            .get(&url)
            .query(&[
                ("actions", actions_param.as_str()),
                ("ids", ids_param.as_str()),
            ])
            .build()
            .map_err(HttpError::from)?;
        self.inner
            .send_request_and_decode(request, "Consent read multiaction failed")
            .await
    }
}

#[cfg(test)]
mod tests {
    use crate::api::consent::{ConsentResult, ConsentValue};
    use crate::test_utils::mock_auth::MockAuthenticator;
    use crate::test_utils::must::Must;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn setup() -> (MockServer, crate::client::ForceClient<MockAuthenticator>) {
        let server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &server.uri());
        let client = crate::client::builder()
            .authenticate(auth)
            .build()
            .await
            .must();
        (server, client)
    }

    // ── read_consent tests ───────────────────────────────────────────

    #[tokio::test]
    async fn test_read_consent_success() {
        let (server, client) = setup().await;

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/consent/action/email"))
            .and(query_param("ids", "001xx000003GYk1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "001xx000003GYk1": {
                    "result": "Success",
                    "proceed": {
                        "email": "Yes"
                    }
                }
            })))
            .mount(&server)
            .await;

        let result = client
            .consent()
            .read_consent("email", &["001xx000003GYk1"])
            .await
            .must();

        assert_eq!(result.len(), 1);
        let record = &result["001xx000003GYk1"];
        assert_eq!(record.result, ConsentResult::Success);
        assert_eq!(record.proceed["email"], ConsentValue::Yes);
    }

    #[tokio::test]
    async fn test_read_consent_multiple_ids() {
        let (server, client) = setup().await;

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/consent/action/email"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "001xx000003GYk1": {
                    "result": "Success",
                    "proceed": {"email": "Yes"}
                },
                "001xx000003GYk2": {
                    "result": "Success",
                    "proceed": {"email": "No"}
                }
            })))
            .mount(&server)
            .await;

        let result = client
            .consent()
            .read_consent("email", &["001xx000003GYk1", "001xx000003GYk2"])
            .await
            .must();

        assert_eq!(result.len(), 2);
        assert_eq!(
            result["001xx000003GYk1"].proceed["email"],
            ConsentValue::Yes
        );
        assert_eq!(result["001xx000003GYk2"].proceed["email"], ConsentValue::No);
    }

    #[tokio::test]
    async fn test_read_consent_not_found_record() {
        let (server, client) = setup().await;

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/consent/action/email"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "001xx000003GONE": {
                    "result": "NotFound",
                    "proceed": {}
                }
            })))
            .mount(&server)
            .await;

        let result = client
            .consent()
            .read_consent("email", &["001xx000003GONE"])
            .await
            .must();

        assert_eq!(result["001xx000003GONE"].result, ConsentResult::NotFound);
        assert!(result["001xx000003GONE"].proceed.is_empty());
    }

    #[tokio::test]
    async fn test_read_consent_server_error() {
        let (server, client) = setup().await;

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/consent/action/email"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let result = client
            .consent()
            .read_consent("email", &["001xx000003GYk1"])
            .await;
        assert!(result.is_err());
    }

    // ── read_consent_multi tests ─────────────────────────────────────

    #[tokio::test]
    async fn test_read_consent_multi_success() {
        let (server, client) = setup().await;

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/consent/multiaction"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "001xx000003GYk1": {
                    "result": "Success",
                    "proceed": {
                        "email": "Yes",
                        "track": "No",
                        "shouldForget": "No"
                    }
                }
            })))
            .mount(&server)
            .await;

        let result = client
            .consent()
            .read_consent_multi(&["email", "track", "shouldForget"], &["001xx000003GYk1"])
            .await
            .must();

        let record = &result["001xx000003GYk1"];
        assert_eq!(record.proceed["email"], ConsentValue::Yes);
        assert_eq!(record.proceed["track"], ConsentValue::No);
        assert_eq!(record.proceed["shouldForget"], ConsentValue::No);
    }

    #[tokio::test]
    async fn test_read_consent_multi_mixed_results() {
        let (server, client) = setup().await;

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/consent/multiaction"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "001xx000003GYk1": {
                    "result": "Success",
                    "proceed": {"email": "Yes", "track": "Yes"}
                },
                "001xx000003GYk2": {
                    "result": "Success",
                    "proceed": {"email": "No", "track": "No"}
                }
            })))
            .mount(&server)
            .await;

        let result = client
            .consent()
            .read_consent_multi(&["email", "track"], &["001xx000003GYk1", "001xx000003GYk2"])
            .await
            .must();

        assert_eq!(result.len(), 2);
        assert_eq!(
            result["001xx000003GYk1"].proceed["email"],
            ConsentValue::Yes
        );
        assert_eq!(result["001xx000003GYk2"].proceed["email"], ConsentValue::No);
    }

    #[tokio::test]
    async fn test_read_consent_multi_error() {
        let (server, client) = setup().await;

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/consent/multiaction"))
            .respond_with(
                ResponseTemplate::new(400).set_body_json(serde_json::json!([{
                    "message": "Invalid action specified",
                    "errorCode": "INVALID_ACTION"
                }])),
            )
            .mount(&server)
            .await;

        let result = client
            .consent()
            .read_consent_multi(&["invalid_action"], &["001xx000003GYk1"])
            .await;
        assert!(result.is_err());
    }

    // ── Handler construction tests ───────────────────────────────────

    #[tokio::test]
    async fn test_handler_is_cloneable() {
        let (_server, client) = setup().await;
        let handler = client.consent();
        let cloned = handler.clone();
        let debug_original = format!("{handler:?}");
        let debug_cloned = format!("{cloned:?}");
        assert_eq!(debug_original, debug_cloned);
    }

    #[tokio::test]
    async fn test_handler_is_debug() {
        let (_server, client) = setup().await;
        let handler = client.consent();
        let debug = format!("{handler:?}");
        assert!(debug.contains("ConsentHandler"));
    }
}
