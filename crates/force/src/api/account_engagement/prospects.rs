//! Account Engagement (Pardot) v5 `prospects` object — full CRUD.
#![allow(clippy::doc_markdown)]

use super::AccountEngagementHandler;
use super::types::QueryResponse;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Object path segment for prospects.
const OBJECT: &str = "objects/prospects";

/// A prospect (lead/contact) in Account Engagement.
///
/// v5 only populates the fields explicitly requested via the `fields` query
/// parameter, so every field is optional. Read-only audit fields (`id`,
/// `created_at`, `updated_at`, `last_activity_at`) are omitted from write
/// bodies. Unknown/unrequested fields are captured in [`Prospect::extra`].
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Prospect {
    /// Account Engagement numeric id (read-only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    /// Email address.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    /// First name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_name: Option<String>,
    /// Last name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_name: Option<String>,
    /// Company / organization.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub company: Option<String>,
    /// Job title.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_title: Option<String>,
    /// Phone number.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone: Option<String>,
    /// Prospect score.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<i64>,
    /// Timestamp of last activity (read-only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_activity_at: Option<String>,
    /// Creation timestamp (read-only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    /// Last-update timestamp (read-only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    /// Any additional fields returned by the API (forward-compatible).
    #[serde(flatten, default)]
    pub extra: HashMap<String, serde_json::Value>,
}

impl<A: crate::auth::Authenticator> AccountEngagementHandler<A> {
    /// Query prospects. `fields` is a comma-delimited list of fields to return
    /// (required by the API to populate data); `extra_params` supplies filters,
    /// `limit`, `orderBy`, or a `nextPageToken`.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response cannot be decoded.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let page = ae
    ///     .query_prospects("id,email,firstName", &[("limit", "50")])
    ///     .await?;
    /// ```
    pub async fn query_prospects(
        &self,
        fields: &str,
        extra_params: &[(&str, &str)],
    ) -> Result<QueryResponse<Prospect>> {
        let mut params = Vec::with_capacity(extra_params.len() + 1);
        params.push(("fields", fields));
        params.extend_from_slice(extra_params);
        self.query_objects(OBJECT, &params).await
    }

    /// Read a single prospect by id, requesting `fields`.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response cannot be decoded.
    pub async fn get_prospect(&self, id: &str, fields: &str) -> Result<Prospect> {
        self.read_object(OBJECT, id, fields).await
    }

    /// Create a prospect.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response cannot be decoded.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let new_prospect = Prospect { email: Some("a@b.com".into()), ..Default::default() };
    /// let created = ae.create_prospect(&new_prospect).await?;
    /// ```
    pub async fn create_prospect(&self, prospect: &Prospect) -> Result<Prospect> {
        self.create_object(OBJECT, prospect).await
    }

    /// Update a prospect by id.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response cannot be decoded.
    pub async fn update_prospect(&self, id: &str, prospect: &Prospect) -> Result<Prospect> {
        self.update_object(OBJECT, id, prospect).await
    }

    /// Delete a prospect by id.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or returns a non-success status.
    pub async fn delete_prospect(&self, id: &str) -> Result<()> {
        self.delete_object(OBJECT, id).await
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
    async fn test_query_prospects_success() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v5/objects/prospects"))
            .and(header("Pardot-Business-Unit-Id", TEST_BU))
            .and(query_param("fields", "id,email,firstName"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "values": [
                    {"id": 1, "email": "jane@example.com", "firstName": "Jane"},
                    {"id": 2, "email": "bob@example.com", "firstName": "Bob"}
                ],
                "nextPageToken": "abc",
                "nextPageUrl": "https://pi.pardot.com/api/v5/objects/prospects?nextPageToken=abc"
            })))
            .expect(1)
            .mount(&server)
            .await;

        let handler = handler_for(&server).await;
        let page = handler
            .query_prospects("id,email,firstName", &[])
            .await
            .must();
        assert_eq!(page.values.len(), 2);
        assert_eq!(page.values[0].email.as_deref(), Some("jane@example.com"));
        assert_eq!(page.next_page_token.as_deref(), Some("abc"));
        assert!(page.next_page_url.is_some());
    }

    #[tokio::test]
    async fn test_query_prospects_forwards_extra_params() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v5/objects/prospects"))
            .and(query_param("fields", "id"))
            .and(query_param("limit", "10"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"values": []})),
            )
            .expect(1)
            .mount(&server)
            .await;

        let handler = handler_for(&server).await;
        let page = handler
            .query_prospects("id", &[("limit", "10")])
            .await
            .must();
        assert!(page.values.is_empty());
        assert!(page.next_page_token.is_none());
    }

    #[tokio::test]
    async fn test_get_prospect_success() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v5/objects/prospects/1"))
            .and(header("Pardot-Business-Unit-Id", TEST_BU))
            .and(query_param("fields", "id,email"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({"id": 1, "email": "jane@example.com", "score": 42}),
            ))
            .mount(&server)
            .await;

        let handler = handler_for(&server).await;
        let prospect = handler.get_prospect("1", "id,email").await.must();
        assert_eq!(prospect.id, Some(1));
        assert_eq!(prospect.score, Some(42));
    }

    #[tokio::test]
    async fn test_create_prospect_sends_body() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v5/objects/prospects"))
            .and(header("Pardot-Business-Unit-Id", TEST_BU))
            .and(body_json(
                serde_json::json!({"email": "new@example.com", "firstName": "New"}),
            ))
            .respond_with(ResponseTemplate::new(201).set_body_json(
                serde_json::json!({"id": 5, "email": "new@example.com", "firstName": "New"}),
            ))
            .expect(1)
            .mount(&server)
            .await;

        let handler = handler_for(&server).await;
        let input = Prospect {
            email: Some("new@example.com".into()),
            first_name: Some("New".into()),
            ..Default::default()
        };
        let created = handler.create_prospect(&input).await.must();
        assert_eq!(created.id, Some(5));
    }

    #[tokio::test]
    async fn test_update_prospect_patches() {
        let server = MockServer::start().await;
        Mock::given(method("PATCH"))
            .and(path("/api/v5/objects/prospects/5"))
            .and(body_json(serde_json::json!({"score": 100})))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"id": 5, "score": 100})),
            )
            .mount(&server)
            .await;

        let handler = handler_for(&server).await;
        let input = Prospect {
            score: Some(100),
            ..Default::default()
        };
        let updated = handler.update_prospect("5", &input).await.must();
        assert_eq!(updated.score, Some(100));
    }

    #[tokio::test]
    async fn test_delete_prospect_204() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(path("/api/v5/objects/prospects/5"))
            .and(header("Pardot-Business-Unit-Id", TEST_BU))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;

        let handler = handler_for(&server).await;
        handler.delete_prospect("5").await.must();
    }

    #[tokio::test]
    async fn test_query_prospects_400_returns_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v5/objects/prospects"))
            .respond_with(
                ResponseTemplate::new(400)
                    .set_body_json(serde_json::json!({"code": 60, "message": "Bad request"})),
            )
            .mount(&server)
            .await;

        let handler = handler_for(&server).await;
        let result = handler.query_prospects("id", &[]).await;
        assert!(result.is_err());
    }
}
