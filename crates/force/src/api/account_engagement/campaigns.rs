//! Account Engagement (Pardot) v5 `campaigns` object — GET + POST only.
#![allow(clippy::doc_markdown)]

use super::AccountEngagementHandler;
use super::types::QueryResponse;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Object path segment for campaigns.
const OBJECT: &str = "objects/campaigns";

/// A campaign in Account Engagement.
///
/// v5 supports read (query + by id) and create only — there is no PATCH or
/// DELETE for campaigns.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Campaign {
    /// Numeric id (read-only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    /// Campaign name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Linked Salesforce campaign id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub salesforce_id: Option<String>,
    /// Campaign cost.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost: Option<f64>,
    /// Containing folder id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folder_id: Option<i64>,
    /// Parent campaign id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_campaign_id: Option<i64>,
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
    /// Query campaigns. `fields` is required to populate data.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response cannot be decoded.
    pub async fn query_campaigns(
        &self,
        fields: &str,
        extra_params: &[(&str, &str)],
    ) -> Result<QueryResponse<Campaign>> {
        let mut params = Vec::with_capacity(extra_params.len() + 1);
        params.push(("fields", fields));
        params.extend_from_slice(extra_params);
        self.query_objects(OBJECT, &params).await
    }

    /// Read a single campaign by id.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response cannot be decoded.
    pub async fn get_campaign(&self, id: &str, fields: &str) -> Result<Campaign> {
        self.read_object(OBJECT, id, fields).await
    }

    /// Create a campaign.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response cannot be decoded.
    pub async fn create_campaign(&self, campaign: &Campaign) -> Result<Campaign> {
        self.create_object(OBJECT, campaign).await
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
    async fn test_query_campaigns_success() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v5/objects/campaigns"))
            .and(header("Pardot-Business-Unit-Id", TEST_BU))
            .and(query_param("fields", "id,name,cost"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "values": [{"id": 3, "name": "Spring", "cost": 1250.5}]
            })))
            .mount(&server)
            .await;

        let handler = handler_for(&server).await;
        let page = handler.query_campaigns("id,name,cost", &[]).await.must();
        assert_eq!(page.values[0].name.as_deref(), Some("Spring"));
        assert_eq!(page.values[0].cost, Some(1250.5));
    }

    #[tokio::test]
    async fn test_create_campaign_sends_body() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v5/objects/campaigns"))
            .and(body_json(serde_json::json!({"name": "Summer"})))
            .respond_with(
                ResponseTemplate::new(201)
                    .set_body_json(serde_json::json!({"id": 4, "name": "Summer"})),
            )
            .mount(&server)
            .await;

        let handler = handler_for(&server).await;
        let input = Campaign {
            name: Some("Summer".into()),
            ..Default::default()
        };
        let created = handler.create_campaign(&input).await.must();
        assert_eq!(created.id, Some(4));
    }
}
