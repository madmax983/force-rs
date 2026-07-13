//! Account Engagement (Pardot) v5 `emails` object — send (POST) + read.
#![allow(clippy::doc_markdown)]

use super::AccountEngagementHandler;
use super::types::QueryResponse;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Object path segment for emails.
const OBJECT: &str = "objects/emails";

/// A one-to-one email in Account Engagement.
///
/// v5 supports send (POST), read-by-id, and query. There is no `createdAt`
/// on emails — use `sent_at` for temporal filtering.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Email {
    /// Numeric id (read-only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    /// Email name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Subject line.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    /// Recipient prospect id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prospect_id: Option<i64>,
    /// Associated campaign id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub campaign_id: Option<i64>,
    /// Source email template id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email_template_id: Option<i64>,
    /// Timestamp the email was sent (read-only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sent_at: Option<String>,
    /// Whether the email is operational (transactional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_operational: Option<bool>,
    /// Any additional fields returned by the API (forward-compatible).
    #[serde(flatten, default)]
    pub extra: HashMap<String, serde_json::Value>,
}

impl<A: crate::auth::Authenticator> AccountEngagementHandler<A> {
    /// Query emails. `fields` is required to populate data.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response cannot be decoded.
    pub async fn query_emails(
        &self,
        fields: &str,
        extra_params: &[(&str, &str)],
    ) -> Result<QueryResponse<Email>> {
        let mut params = Vec::with_capacity(extra_params.len() + 1);
        params.push(("fields", fields));
        params.extend_from_slice(extra_params);
        self.query_objects(OBJECT, &params).await
    }

    /// Read a single email by id.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response cannot be decoded.
    pub async fn get_email(&self, id: &str, fields: &str) -> Result<Email> {
        self.read_object(OBJECT, id, fields).await
    }

    /// Send a one-to-one email (POST).
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response cannot be decoded.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let email = Email {
    ///     prospect_id: Some(5),
    ///     email_template_id: Some(42),
    ///     ..Default::default()
    /// };
    /// let sent = ae.send_email(&email).await?;
    /// ```
    pub async fn send_email(&self, email: &Email) -> Result<Email> {
        self.create_object(OBJECT, email).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::mock_auth::MockAuthenticator;
    use crate::test_utils::must::Must;
    use wiremock::matchers::{body_json, header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const TEST_BU: &str = "0Uv000000000001AAA";

    async fn handler_for(server: &MockServer) -> AccountEngagementHandler<MockAuthenticator> {
        let auth = MockAuthenticator::new("test_token", &server.uri());
        let client = crate::client::builder()
            .authenticate(auth)
            .build()
            .await
            .must();
        client.account_engagement(TEST_BU).with_host(server.uri())
    }

    #[tokio::test]
    async fn test_query_emails_success() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v5/objects/emails"))
            .and(query_param("fields", "id,subject,sentAt"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "values": [{"id": 20, "subject": "Welcome", "sentAt": "2026-01-01T00:00:00Z"}]
            })))
            .mount(&server)
            .await;

        let handler = handler_for(&server).await;
        let page = handler.query_emails("id,subject,sentAt", &[]).await.must();
        assert_eq!(page.values[0].subject.as_deref(), Some("Welcome"));
    }

    #[tokio::test]
    async fn test_send_email_posts_body() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v5/objects/emails"))
            .and(header("Pardot-Business-Unit-Id", TEST_BU))
            .and(body_json(
                serde_json::json!({"prospectId": 5, "emailTemplateId": 42}),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({"id": 21, "prospectId": 5, "emailTemplateId": 42}),
            ))
            .expect(1)
            .mount(&server)
            .await;

        let handler = handler_for(&server).await;
        let input = Email {
            prospect_id: Some(5),
            email_template_id: Some(42),
            ..Default::default()
        };
        let sent = handler.send_email(&input).await.must();
        assert_eq!(sent.id, Some(21));
    }
}
