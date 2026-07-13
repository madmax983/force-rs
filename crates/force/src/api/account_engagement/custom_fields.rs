//! Account Engagement (Pardot) v5 `custom-fields` object — full CRUD.
#![allow(clippy::doc_markdown)]

use super::AccountEngagementHandler;
use super::types::QueryResponse;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Object path segment for custom fields (hyphenated).
const OBJECT: &str = "objects/custom-fields";

/// A prospect custom field definition in Account Engagement.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CustomField {
    /// Numeric id (read-only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    /// Display name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// User-defined API field id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field_id: Option<String>,
    /// Field type (e.g. Text, Dropdown, Checkbox).
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub type_: Option<String>,
    /// Whether the field is required.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_required: Option<bool>,
    /// Linked Salesforce field id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub salesforce_id: Option<String>,
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
    /// Query custom fields. `fields` is required to populate data.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response cannot be decoded.
    pub async fn query_custom_fields(
        &self,
        fields: &str,
        extra_params: &[(&str, &str)],
    ) -> Result<QueryResponse<CustomField>> {
        let mut params = Vec::with_capacity(extra_params.len() + 1);
        params.push(("fields", fields));
        params.extend_from_slice(extra_params);
        self.query_objects(OBJECT, &params).await
    }

    /// Read a single custom field by id.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response cannot be decoded.
    pub async fn get_custom_field(&self, id: &str, fields: &str) -> Result<CustomField> {
        self.read_object(OBJECT, id, fields).await
    }

    /// Create a custom field.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response cannot be decoded.
    pub async fn create_custom_field(&self, field: &CustomField) -> Result<CustomField> {
        self.create_object(OBJECT, field).await
    }

    /// Update a custom field by id.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response cannot be decoded.
    pub async fn update_custom_field(&self, id: &str, field: &CustomField) -> Result<CustomField> {
        self.update_object(OBJECT, id, field).await
    }

    /// Delete a custom field by id.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or returns a non-success status.
    pub async fn delete_custom_field(&self, id: &str) -> Result<()> {
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
    async fn test_query_custom_fields_success_type_field() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v5/objects/custom-fields"))
            .and(query_param("fields", "id,name,type"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "values": [{"id": 8, "name": "Industry", "type": "Dropdown"}]
            })))
            .mount(&server)
            .await;

        let handler = handler_for(&server).await;
        let page = handler
            .query_custom_fields("id,name,type", &[])
            .await
            .must();
        assert_eq!(page.values[0].type_.as_deref(), Some("Dropdown"));
    }

    #[tokio::test]
    async fn test_create_custom_field_serializes_type() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v5/objects/custom-fields"))
            .and(body_json(
                serde_json::json!({"name": "Region", "fieldId": "region", "type": "Text"}),
            ))
            .respond_with(
                ResponseTemplate::new(201)
                    .set_body_json(serde_json::json!({"id": 9, "name": "Region", "type": "Text"})),
            )
            .mount(&server)
            .await;

        let handler = handler_for(&server).await;
        let input = CustomField {
            name: Some("Region".into()),
            field_id: Some("region".into()),
            type_: Some("Text".into()),
            ..Default::default()
        };
        let created = handler.create_custom_field(&input).await.must();
        assert_eq!(created.id, Some(9));
    }
}
