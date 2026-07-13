//! Account Engagement (Pardot) v5 `forms` object — GET / POST / DELETE.
#![allow(clippy::doc_markdown)]

use super::AccountEngagementHandler;
use super::types::QueryResponse;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Object path segment for forms.
const OBJECT: &str = "objects/forms";

/// A form in Account Engagement.
///
/// v5 supports read (query + by id), create, and delete — there is no PATCH.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Form {
    /// Numeric id (read-only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    /// Form name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Associated campaign id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub campaign_id: Option<i64>,
    /// Containing folder id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folder_id: Option<i64>,
    /// HTML embed code (read-only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embed_code: Option<String>,
    /// Whether the form is soft-deleted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_deleted: Option<bool>,
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
    /// Query forms. `fields` is required to populate data.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response cannot be decoded.
    pub async fn query_forms(
        &self,
        fields: &str,
        extra_params: &[(&str, &str)],
    ) -> Result<QueryResponse<Form>> {
        let mut params = Vec::with_capacity(extra_params.len() + 1);
        params.push(("fields", fields));
        params.extend_from_slice(extra_params);
        self.query_objects(OBJECT, &params).await
    }

    /// Read a single form by id.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response cannot be decoded.
    pub async fn get_form(&self, id: &str, fields: &str) -> Result<Form> {
        self.read_object(OBJECT, id, fields).await
    }

    /// Create a form.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response cannot be decoded.
    pub async fn create_form(&self, form: &Form) -> Result<Form> {
        self.create_object(OBJECT, form).await
    }

    /// Delete a form by id.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or returns a non-success status.
    pub async fn delete_form(&self, id: &str) -> Result<()> {
        self.delete_object(OBJECT, id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::mock_auth::MockAuthenticator;
    use crate::test_utils::must::Must;
    use wiremock::matchers::{body_json, method, path, query_param};
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
    async fn test_query_forms_success() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v5/objects/forms"))
            .and(query_param("fields", "id,name"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "values": [{"id": 12, "name": "Contact Us"}]
            })))
            .mount(&server)
            .await;

        let handler = handler_for(&server).await;
        let page = handler.query_forms("id,name", &[]).await.must();
        assert_eq!(page.values[0].name.as_deref(), Some("Contact Us"));
    }

    #[tokio::test]
    async fn test_create_form_sends_body() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v5/objects/forms"))
            .and(body_json(serde_json::json!({"name": "Signup"})))
            .respond_with(
                ResponseTemplate::new(201)
                    .set_body_json(serde_json::json!({"id": 13, "name": "Signup"})),
            )
            .mount(&server)
            .await;

        let handler = handler_for(&server).await;
        let input = Form {
            name: Some("Signup".into()),
            ..Default::default()
        };
        let created = handler.create_form(&input).await.must();
        assert_eq!(created.id, Some(13));
    }

    #[tokio::test]
    async fn test_delete_form_204() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(path("/api/v5/objects/forms/13"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;

        let handler = handler_for(&server).await;
        handler.delete_form("13").await.must();
    }
}
