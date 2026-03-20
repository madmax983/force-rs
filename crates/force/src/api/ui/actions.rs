//! UI API action endpoints.
//!
//! Provides record actions (standard and custom buttons/actions) via the
//! Salesforce UI API (`/services/data/vXX.0/ui-api/actions/record/{ids}`).

#![allow(clippy::doc_markdown)]

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

// ─── Response types ──────────────────────────────────────────────────────────

/// The full actions response for one or more records.
///
/// Returned by `record_actions()`. Keys in `actions` are record IDs mapping
/// to a list of available action representations for that record.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordActionRepresentation {
    /// Map from record ID to the list of available actions.
    pub actions: HashMap<String, Vec<ActionRepresentation>>,
    /// Any additional fields returned by the API.
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// A single action available for a record.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionRepresentation {
    /// The API name of the action (e.g., `"NewCase"`).
    pub api_name: String,
    /// Human-readable label (e.g., `"New Case"`).
    pub label: String,
    /// The action type (e.g., `"StandardButton"`, `"CustomButton"`, `"QuickAction"`).
    pub action_type: String,
    /// Whether the action supports mass (multi-record) invocation.
    pub is_massable: bool,
    /// Any additional fields returned by the API.
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

// ─── UiHandler<A> implementation ─────────────────────────────────────────────

impl<A: crate::auth::Authenticator> crate::api::ui::UiHandler<A> {
    /// Returns the available actions for one or more Salesforce records.
    ///
    /// Calls `GET /ui-api/actions/record/{ids}` where `ids` is a
    /// comma-joined list of Salesforce record IDs.
    ///
    /// # Errors
    ///
    /// Returns an error if any ID is invalid or the HTTP request fails.
    pub async fn record_actions(
        &self,
        ids: &[&str],
    ) -> crate::error::Result<RecordActionRepresentation> {
        let ids_str = ids.join(",");
        let path = format!("actions/record/{ids_str}");
        self.get(&path, None, "Failed to fetch record actions")
            .await
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use crate::client::builder;
    use crate::test_support::{MockAuthenticator, Must};
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const VALID_ID: &str = "001000000000001AAA";
    const VALID_ID2: &str = "001000000000002AAA";

    async fn make_client(server: &MockServer) -> crate::client::ForceClient<MockAuthenticator> {
        let auth = MockAuthenticator::new("test_token", &server.uri());
        builder().authenticate(auth).build().await.must()
    }

    fn action_json(api_name: &str) -> serde_json::Value {
        json!({
            "apiName": api_name,
            "label": api_name,
            "actionType": "StandardButton",
            "isMassable": false
        })
    }

    // ── record_actions single ID ──────────────────────────────────────────────

    #[tokio::test]
    async fn test_record_actions_single_id_success() {
        let server = MockServer::start().await;
        let client = make_client(&server).await;

        let response_body = json!({
            "actions": {
                VALID_ID: [
                    action_json("Edit"),
                    action_json("Delete")
                ]
            }
        });

        Mock::given(method("GET"))
            .and(path(format!(
                "/services/data/v60.0/ui-api/actions/record/{VALID_ID}"
            )))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
            .expect(1)
            .mount(&server)
            .await;

        let result = client.ui().record_actions(&[VALID_ID]).await.must();

        assert!(result.actions.contains_key(VALID_ID));
        let actions = &result.actions[VALID_ID];
        assert_eq!(actions.len(), 2);
        assert_eq!(actions[0].api_name, "Edit");
        assert_eq!(actions[1].api_name, "Delete");
    }

    // ── record_actions multiple IDs ───────────────────────────────────────────

    #[tokio::test]
    async fn test_record_actions_multiple_ids_success() {
        let server = MockServer::start().await;
        let client = make_client(&server).await;

        let joined = format!("{VALID_ID},{VALID_ID2}");
        let response_body = json!({
            "actions": {
                VALID_ID:  [action_json("Edit")],
                VALID_ID2: [action_json("Delete")]
            }
        });

        Mock::given(method("GET"))
            .and(path(format!(
                "/services/data/v60.0/ui-api/actions/record/{joined}"
            )))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
            .expect(1)
            .mount(&server)
            .await;

        let result = client
            .ui()
            .record_actions(&[VALID_ID, VALID_ID2])
            .await
            .must();

        assert_eq!(result.actions.len(), 2);
        assert!(result.actions.contains_key(VALID_ID));
        assert!(result.actions.contains_key(VALID_ID2));
    }

    // ── record_actions 404 ────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_record_actions_not_found() {
        let server = MockServer::start().await;
        let client = make_client(&server).await;

        Mock::given(method("GET"))
            .and(path(format!(
                "/services/data/v60.0/ui-api/actions/record/{VALID_ID}"
            )))
            .respond_with(ResponseTemplate::new(404).set_body_json(json!([{
                "errorCode": "NOT_FOUND",
                "message": "Record not found"
            }])))
            .expect(1)
            .mount(&server)
            .await;

        let result = client.ui().record_actions(&[VALID_ID]).await;
        assert!(result.is_err());
    }

    // ── unit: deserialize RecordActionRepresentation ─────────────────────────

    #[test]
    fn test_record_action_representation_deserialize() {
        let json_str = r#"{
            "actions": {
                "001000000000001AAA": [
                    {
                        "apiName": "NewTask",
                        "label": "New Task",
                        "actionType": "QuickAction",
                        "isMassable": true
                    }
                ]
            }
        }"#;

        let rep: RecordActionRepresentation = serde_json::from_str(json_str).unwrap();
        let actions = rep.actions.get("001000000000001AAA").unwrap();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].api_name, "NewTask");
        assert_eq!(actions[0].action_type, "QuickAction");
        assert!(actions[0].is_massable);
    }
}
