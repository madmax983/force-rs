//! Account Engagement (Pardot) v5 `lists` and `list-memberships` objects — full CRUD.
#![allow(clippy::doc_markdown)]

use super::AccountEngagementHandler;
use super::types::QueryResponse;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Object path segment for lists.
const LISTS: &str = "objects/lists";
/// Object path segment for list memberships (hyphenated).
const LIST_MEMBERSHIPS: &str = "objects/list-memberships";

/// A prospect list in Account Engagement.
///
/// v5 only populates requested fields, so every field is optional.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct List {
    /// Numeric id (read-only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    /// Internal list name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Public-facing title.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Whether the list is public.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_public: Option<bool>,
    /// Whether the list is dynamic.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_dynamic: Option<bool>,
    /// Containing folder id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folder_id: Option<i64>,
    /// Associated campaign id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub campaign_id: Option<i64>,
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

/// A prospect's membership in a list.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ListMembership {
    /// Numeric id (read-only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    /// The list this membership belongs to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list_id: Option<i64>,
    /// The prospect this membership belongs to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prospect_id: Option<i64>,
    /// Whether the prospect opted out.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opted_out: Option<bool>,
    /// Whether the membership is soft-deleted.
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
    /// Query lists. `fields` is required to populate data.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response cannot be decoded.
    pub async fn query_lists(
        &self,
        fields: &str,
        extra_params: &[(&str, &str)],
    ) -> Result<QueryResponse<List>> {
        let mut params = Vec::with_capacity(extra_params.len() + 1);
        params.push(("fields", fields));
        params.extend_from_slice(extra_params);
        self.query_objects(LISTS, &params).await
    }

    /// Read a single list by id.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response cannot be decoded.
    pub async fn get_list(&self, id: &str, fields: &str) -> Result<List> {
        self.read_object(LISTS, id, fields).await
    }

    /// Create a list.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response cannot be decoded.
    pub async fn create_list(&self, list: &List) -> Result<List> {
        self.create_object(LISTS, list).await
    }

    /// Update a list by id.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response cannot be decoded.
    pub async fn update_list(&self, id: &str, list: &List) -> Result<List> {
        self.update_object(LISTS, id, list).await
    }

    /// Delete a list by id.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or returns a non-success status.
    pub async fn delete_list(&self, id: &str) -> Result<()> {
        self.delete_object(LISTS, id).await
    }

    /// Query list memberships. `fields` is required to populate data.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response cannot be decoded.
    pub async fn query_list_memberships(
        &self,
        fields: &str,
        extra_params: &[(&str, &str)],
    ) -> Result<QueryResponse<ListMembership>> {
        let mut params = Vec::with_capacity(extra_params.len() + 1);
        params.push(("fields", fields));
        params.extend_from_slice(extra_params);
        self.query_objects(LIST_MEMBERSHIPS, &params).await
    }

    /// Read a single list membership by id.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response cannot be decoded.
    pub async fn get_list_membership(&self, id: &str, fields: &str) -> Result<ListMembership> {
        self.read_object(LIST_MEMBERSHIPS, id, fields).await
    }

    /// Create a list membership.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response cannot be decoded.
    pub async fn create_list_membership(
        &self,
        membership: &ListMembership,
    ) -> Result<ListMembership> {
        self.create_object(LIST_MEMBERSHIPS, membership).await
    }

    /// Update a list membership by id.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response cannot be decoded.
    pub async fn update_list_membership(
        &self,
        id: &str,
        membership: &ListMembership,
    ) -> Result<ListMembership> {
        self.update_object(LIST_MEMBERSHIPS, id, membership).await
    }

    /// Delete a list membership by id.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or returns a non-success status.
    pub async fn delete_list_membership(&self, id: &str) -> Result<()> {
        self.delete_object(LIST_MEMBERSHIPS, id).await
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
    async fn test_query_lists_success() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v5/objects/lists"))
            .and(header("Pardot-Business-Unit-Id", TEST_BU))
            .and(query_param("fields", "id,name"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "values": [{"id": 10, "name": "Newsletter", "isDynamic": false}]
            })))
            .mount(&server)
            .await;

        let handler = handler_for(&server).await;
        let page = handler.query_lists("id,name", &[]).await.must();
        assert_eq!(page.values.len(), 1);
        assert_eq!(page.values[0].name.as_deref(), Some("Newsletter"));
        assert_eq!(page.values[0].is_dynamic, Some(false));
    }

    #[tokio::test]
    async fn test_create_list_sends_body() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v5/objects/lists"))
            .and(body_json(serde_json::json!({"name": "Promo"})))
            .respond_with(
                ResponseTemplate::new(201)
                    .set_body_json(serde_json::json!({"id": 11, "name": "Promo"})),
            )
            .mount(&server)
            .await;

        let handler = handler_for(&server).await;
        let input = List {
            name: Some("Promo".into()),
            ..Default::default()
        };
        let created = handler.create_list(&input).await.must();
        assert_eq!(created.id, Some(11));
    }

    #[tokio::test]
    async fn test_query_list_memberships_hyphenated_path() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v5/objects/list-memberships"))
            .and(query_param("fields", "id,listId,prospectId"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "values": [{"id": 1, "listId": 10, "prospectId": 5, "optedOut": false}]
            })))
            .mount(&server)
            .await;

        let handler = handler_for(&server).await;
        let page = handler
            .query_list_memberships("id,listId,prospectId", &[])
            .await
            .must();
        assert_eq!(page.values[0].list_id, Some(10));
        assert_eq!(page.values[0].prospect_id, Some(5));
    }

    #[tokio::test]
    async fn test_create_list_membership() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v5/objects/list-memberships"))
            .and(body_json(
                serde_json::json!({"listId": 10, "prospectId": 5}),
            ))
            .respond_with(
                ResponseTemplate::new(201)
                    .set_body_json(serde_json::json!({"id": 77, "listId": 10, "prospectId": 5})),
            )
            .mount(&server)
            .await;

        let handler = handler_for(&server).await;
        let input = ListMembership {
            list_id: Some(10),
            prospect_id: Some(5),
            ..Default::default()
        };
        let created = handler.create_list_membership(&input).await.must();
        assert_eq!(created.id, Some(77));
    }

    #[tokio::test]
    async fn test_delete_list_membership_204() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(path("/api/v5/objects/list-memberships/77"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;

        let handler = handler_for(&server).await;
        handler.delete_list_membership("77").await.must();
    }
}
