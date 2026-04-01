//! UI API layout endpoints.
//!
//! Provides page layout structures for SObjects via the Salesforce UI API
//! (`/services/data/vXX.0/ui-api/layout/`).

#![allow(clippy::doc_markdown)]

use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;

// ─── Response types ───────────────────────────────────────────────────────────

/// A page layout for a Salesforce SObject.
///
/// Returned by `layout()`. Contains sections, rows, and items that describe
/// how fields are arranged on a record page.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordLayoutRepresentation {
    /// Unique identifier for this layout.
    pub id: String,
    /// The layout type string (e.g., `"Full"`, `"Compact"`).
    pub layout_type: String,
    /// The interaction mode string (e.g., `"View"`, `"Edit"`, `"Create"`).
    pub mode: String,
    /// Ordered list of layout sections.
    pub sections: Vec<LayoutSection>,
    /// Any additional fields returned by the API.
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// A section within a page layout.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutSection {
    /// `true` if the section is collapsed by default.
    pub collapsed: bool,
    /// Number of columns in this section.
    pub columns: u32,
    /// Optional display heading for this section.
    pub heading: Option<String>,
    /// Unique identifier for this section.
    pub id: String,
    /// Ordered rows within this section.
    pub layout_rows: Vec<LayoutRow>,
    /// Number of rows in this section.
    pub rows: u32,
    /// `true` if the section heading should be displayed.
    pub use_heading: bool,
    /// Any additional fields returned by the API.
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// A single row within a layout section.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutRow {
    /// Items (field slots) within this row.
    pub layout_items: Vec<LayoutItem>,
    /// Any additional fields returned by the API.
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// A single item (field slot) within a layout row.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutItem {
    /// Field API name, if this slot contains a field.
    pub field: Option<String>,
    /// Display label for this item.
    pub label: Option<String>,
    /// `true` if a value is required in this slot.
    pub required: bool,
    /// `true` if the field is sortable.
    pub sortable: bool,
    /// Any additional fields returned by the API.
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

// ─── UiHandler<A> implementation ─────────────────────────────────────────────

impl<A: crate::auth::Authenticator> crate::api::ui::UiHandler<A> {
    /// Returns the page layout for a Salesforce SObject.
    ///
    /// Calls `GET /ui-api/layout/{object}` with optional `layoutType` and
    /// `mode` query parameters.
    ///
    /// * `object` – the SObject API name (e.g., `"Account"`).
    /// * `layout_type` – optional layout type filter (`Compact` or `Full`).
    /// * `mode` – optional interaction mode filter (`Create`, `Edit`, or `View`).
    ///
    /// # Errors
    ///
    /// Returns an error if the object type is not found or the request fails.
    pub async fn layout(
        &self,
        object: &str,
        layout_type: Option<&crate::api::ui::types::LayoutType>,
        mode: Option<&crate::api::ui::types::Mode>,
    ) -> crate::error::Result<RecordLayoutRepresentation> {
        let path = format!("layout/{object}");

        let mut params: Vec<(&str, &str)> = Vec::new();

        let lt_str;
        if let Some(lt) = layout_type {
            lt_str = lt.as_str();
            params.push(("layoutType", lt_str));
        }

        let mode_str;
        if let Some(m) = mode {
            mode_str = m.as_str();
            params.push(("mode", mode_str));
        }

        let query = if params.is_empty() {
            None
        } else {
            Some(params.as_slice())
        };

        self.get(&path, query, "Failed to fetch layout").await
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::ui::types::{LayoutType, Mode};
    use crate::client::builder;
    use crate::test_support::{MockAuthenticator, Must};
    use serde_json::json;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn make_client(server: &MockServer) -> crate::client::ForceClient<MockAuthenticator> {
        let auth = MockAuthenticator::new("test_token", &server.uri());
        builder().authenticate(auth).build().await.must()
    }

    fn minimal_layout_json() -> serde_json::Value {
        json!({
            "id": "layout-001",
            "layoutType": "Full",
            "mode": "View",
            "sections": [
                {
                    "collapsed": false,
                    "columns": 2,
                    "heading": "Account Information",
                    "id": "section-001",
                    "layoutRows": [
                        {
                            "layoutItems": [
                                {
                                    "field": "Name",
                                    "label": "Account Name",
                                    "required": true,
                                    "sortable": true
                                },
                                {
                                    "field": "Phone",
                                    "label": "Phone",
                                    "required": false,
                                    "sortable": false
                                }
                            ]
                        }
                    ],
                    "rows": 1,
                    "useHeading": true
                }
            ]
        })
    }

    // ── layout ───────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_layout_success_no_params() {
        let server = MockServer::start().await;
        let client = make_client(&server).await;

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/ui-api/layout/Account"))
            .respond_with(ResponseTemplate::new(200).set_body_json(minimal_layout_json()))
            .expect(1)
            .mount(&server)
            .await;

        let layout = client.ui().layout("Account", None, None).await.must();

        assert_eq!(layout.id, "layout-001");
        assert_eq!(layout.layout_type, "Full");
        assert_eq!(layout.mode, "View");
        assert_eq!(layout.sections.len(), 1);

        let section = &layout.sections[0];
        assert_eq!(section.id, "section-001");
        assert_eq!(section.columns, 2);
        assert!(!section.collapsed);
        assert!(section.use_heading);
        assert_eq!(section.heading.as_deref(), Some("Account Information"));
        assert_eq!(section.layout_rows.len(), 1);

        let row = &section.layout_rows[0];
        assert_eq!(row.layout_items.len(), 2);

        let item = &row.layout_items[0];
        assert_eq!(item.field.as_deref(), Some("Name"));
        assert!(item.required);
        assert!(item.sortable);
    }

    #[tokio::test]
    async fn test_layout_with_layout_type_param() {
        let server = MockServer::start().await;
        let client = make_client(&server).await;

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/ui-api/layout/Account"))
            .and(query_param("layoutType", "Compact"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": "layout-compact-001",
                "layoutType": "Compact",
                "mode": "View",
                "sections": []
            })))
            .expect(1)
            .mount(&server)
            .await;

        let layout = client
            .ui()
            .layout("Account", Some(&LayoutType::Compact), None)
            .await
            .must();

        assert_eq!(layout.layout_type, "Compact");
    }

    #[tokio::test]
    async fn test_layout_with_both_params() {
        let server = MockServer::start().await;
        let client = make_client(&server).await;

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/ui-api/layout/Contact"))
            .and(query_param("layoutType", "Full"))
            .and(query_param("mode", "Edit"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": "layout-edit-001",
                "layoutType": "Full",
                "mode": "Edit",
                "sections": []
            })))
            .expect(1)
            .mount(&server)
            .await;

        let layout = client
            .ui()
            .layout("Contact", Some(&LayoutType::Full), Some(&Mode::Edit))
            .await
            .must();

        assert_eq!(layout.mode, "Edit");
        assert_eq!(layout.layout_type, "Full");
    }

    #[tokio::test]
    async fn test_layout_not_found() {
        let server = MockServer::start().await;
        let client = make_client(&server).await;

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/ui-api/layout/NoSuchObject"))
            .respond_with(ResponseTemplate::new(404).set_body_json(json!([{
                "errorCode": "NOT_FOUND",
                "message": "The requested resource does not exist"
            }])))
            .expect(1)
            .mount(&server)
            .await;

        let result = client.ui().layout("NoSuchObject", None, None).await;
        let Err(err) = result else {
            panic!("Expected an error");
        };
        assert!(
            matches!(
                err,
                crate::error::ForceError::Api(_) | crate::error::ForceError::Http(_)
            ),
            "Expected Api or Http error, got: {err}"
        );
    }

    // ── unit / deserialization tests ─────────────────────────────────────────

    #[test]
    fn test_record_layout_representation_deserialize() {
        let json_str = r#"{
            "id": "layout-test-001",
            "layoutType": "Full",
            "mode": "View",
            "sections": [
                {
                    "collapsed": false,
                    "columns": 1,
                    "heading": null,
                    "id": "section-a",
                    "layoutRows": [
                        {
                            "layoutItems": [
                                {
                                    "field": "Name",
                                    "label": "Name",
                                    "required": true,
                                    "sortable": false
                                }
                            ]
                        }
                    ],
                    "rows": 1,
                    "useHeading": false
                }
            ]
        }"#;

        let layout: RecordLayoutRepresentation = serde_json::from_str(json_str).must();
        assert_eq!(layout.id, "layout-test-001");
        assert_eq!(layout.layout_type, "Full");
        assert_eq!(layout.mode, "View");
        assert_eq!(layout.sections.len(), 1);

        let section = &layout.sections[0];
        assert_eq!(section.columns, 1);
        assert!(!section.collapsed);
        assert!(!section.use_heading);
        assert!(section.heading.is_none());
        assert_eq!(section.rows, 1);

        let item = &section.layout_rows[0].layout_items[0];
        assert_eq!(item.field.as_deref(), Some("Name"));
        assert!(item.required);
        assert!(!item.sortable);
    }
}
