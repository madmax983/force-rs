//! UI API list view endpoints.
//!
//! Provides list view data, records, and metadata via the Salesforce UI API
//! (`/services/data/vXX.0/ui-api/list-ui/`, `list-records/`, `list-info/`).

#![allow(clippy::doc_markdown)]

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

// ─── Response types ───────────────────────────────────────────────────────────

/// Aggregated list view data combining list info and list records.
///
/// Returned by `list_ui()`. Contains metadata about the list view and the
/// actual record data in a single response.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListUiRepresentation {
    /// Metadata about the list view (columns, name, etc.).
    pub list_info: ListInfoRepresentation,
    /// The actual records returned by the list view.
    pub records: ListRecordsRepresentation,
    /// Any additional fields returned by the API.
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// A paginated collection of list view summaries for an object.
///
/// Returned by `list_views()`. Each entry is a brief summary of one list view.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListViewSummaryCollection {
    /// Total number of list views available.
    pub count: u32,
    /// Pagination token for the current page.
    pub current_page_token: Option<String>,
    /// Pagination token for the next page, if more results exist.
    pub next_page_token: Option<String>,
    /// Pagination token for the previous page, if applicable.
    pub previous_page_token: Option<String>,
    /// The list view summaries for this page.
    pub list_views: Vec<ListViewSummary>,
    /// Any additional fields returned by the API.
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// A brief summary of a single list view.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListViewSummary {
    /// Developer API name of the list view.
    pub api_name: String,
    /// Salesforce ID of the list view.
    pub id: String,
    /// User-visible label.
    pub label: String,
    /// Type of list view (e.g., `"AllList"`, `"MyList"`).
    pub list_view_type: Option<String>,
    /// Visibility setting (e.g., `"Everyone"`, `"Mine"`).
    pub visibility: Option<String>,
    /// Any additional fields returned by the API.
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// Paginated list of records returned by a list view.
///
/// Returned by `list_records()`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListRecordsRepresentation {
    /// Number of records in this page.
    pub count: u32,
    /// Pagination token for the current page.
    pub current_page_token: Option<String>,
    /// Pagination token for the next page, if more results exist.
    pub next_page_token: Option<String>,
    /// Pagination token for the previous page, if applicable.
    pub previous_page_token: Option<String>,
    /// The records for this page.
    pub records: Vec<Value>,
    /// Any additional fields returned by the API.
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// Metadata about a list view: columns, name, and display configuration.
///
/// Returned by `list_info()` and embedded in `ListUiRepresentation`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListInfoRepresentation {
    /// Salesforce ID of the list view.
    pub id: String,
    /// Developer API name of the list view.
    pub api_name: String,
    /// User-visible label.
    pub label: String,
    /// Type of list view (e.g., `"AllList"`, `"MyList"`).
    pub list_view_type: Option<String>,
    /// Column metadata for the list view.
    pub display_columns: Vec<Value>,
    /// Any additional fields returned by the API.
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

// ─── UiHandler<A> implementation ─────────────────────────────────────────────

impl<A: crate::auth::Authenticator> crate::api::ui::UiHandler<A> {
    /// Returns aggregated list view data (info + records) for a specific list view.
    ///
    /// Calls `GET /ui-api/list-ui/{list_view_id}`.
    ///
    /// * `list_view_id` – Salesforce ID of the list view.
    ///
    /// # Errors
    ///
    /// Returns an error if the list view is not found or the request fails.
    pub async fn list_ui(&self, list_view_id: &str) -> crate::error::Result<ListUiRepresentation> {
        let path = format!("list-ui/{list_view_id}");
        self.get(&path, None, "Failed to fetch list UI").await
    }

    /// Returns a paginated collection of list view summaries for an object.
    ///
    /// Calls `GET /ui-api/list-ui/{objectApiName}`.
    ///
    /// * `object` – SObject API name (e.g., `"Account"`, `"Contact"`).
    ///
    /// # Errors
    ///
    /// Returns an error if the object is unknown or the request fails.
    pub async fn list_views(
        &self,
        object: &str,
    ) -> crate::error::Result<ListViewSummaryCollection> {
        let path = format!("list-ui/{object}");
        self.get(&path, None, "Failed to fetch list views").await
    }

    /// Returns a paginated set of records for a given list view.
    ///
    /// Calls `GET /ui-api/list-records/{list_view_id}`.
    ///
    /// * `list_view_id` – Salesforce ID of the list view.
    /// * `page_size` – optional number of records per page.
    /// * `page_token` – optional pagination token for subsequent pages.
    ///
    /// # Errors
    ///
    /// Returns an error if the list view is not found or the request fails.
    pub async fn list_records(
        &self,
        list_view_id: &str,
        page_size: Option<u32>,
        page_token: Option<&str>,
    ) -> crate::error::Result<ListRecordsRepresentation> {
        let path = format!("list-records/{list_view_id}");

        let page_size_str;
        let mut params: Vec<(&str, &str)> = Vec::new();

        if let Some(ps) = page_size {
            page_size_str = ps.to_string();
            params.push(("pageSize", &page_size_str));
        }
        if let Some(pt) = page_token {
            params.push(("pageToken", pt));
        }

        let query = if params.is_empty() {
            None
        } else {
            Some(params.as_slice())
        };

        self.get(&path, query, "Failed to fetch list records").await
    }

    /// Returns metadata about a list view, including display columns.
    ///
    /// Calls `GET /ui-api/list-info/{list_view_id}`.
    ///
    /// * `list_view_id` – Salesforce ID of the list view.
    ///
    /// # Errors
    ///
    /// Returns an error if the list view is not found or the request fails.
    pub async fn list_info(
        &self,
        list_view_id: &str,
    ) -> crate::error::Result<ListInfoRepresentation> {
        let path = format!("list-info/{list_view_id}");
        self.get(&path, None, "Failed to fetch list info").await
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use crate::client::builder;
    use crate::test_support::{MockAuthenticator, Must};
    use serde_json::json;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// A valid-looking list view ID for tests (18-char Salesforce ID format).
    const LIST_VIEW_ID: &str = "00B000000000001AAA";
    /// A second list view ID for variety.
    const LIST_VIEW_ID2: &str = "00B000000000002AAA";

    async fn make_client(server: &MockServer) -> crate::client::ForceClient<MockAuthenticator> {
        let auth = MockAuthenticator::new("test_token", &server.uri());
        builder().authenticate(auth).build().await.must()
    }

    fn minimal_list_info_json(id: &str) -> serde_json::Value {
        json!({
            "id": id,
            "apiName": "AllAccounts",
            "label": "All Accounts",
            "listViewType": "AllList",
            "displayColumns": []
        })
    }

    fn minimal_list_records_json(count: u32) -> serde_json::Value {
        json!({
            "count": count,
            "currentPageToken": null,
            "nextPageToken": null,
            "previousPageToken": null,
            "records": []
        })
    }

    // ── list_ui ───────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_list_ui_success() {
        let server = MockServer::start().await;
        let client = make_client(&server).await;

        let response_body = json!({
            "listInfo": minimal_list_info_json(LIST_VIEW_ID),
            "records": minimal_list_records_json(2)
        });

        Mock::given(method("GET"))
            .and(path(format!(
                "/services/data/v60.0/ui-api/list-ui/{LIST_VIEW_ID}"
            )))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
            .expect(1)
            .mount(&server)
            .await;

        let result = client.ui().list_ui(LIST_VIEW_ID).await.must();

        assert_eq!(result.list_info.id, LIST_VIEW_ID);
        assert_eq!(result.list_info.api_name, "AllAccounts");
        assert_eq!(result.list_info.label, "All Accounts");
        assert_eq!(result.records.count, 2);
    }

    #[tokio::test]
    async fn test_list_ui_not_found() {
        let server = MockServer::start().await;
        let client = make_client(&server).await;

        Mock::given(method("GET"))
            .and(path(format!(
                "/services/data/v60.0/ui-api/list-ui/{LIST_VIEW_ID}"
            )))
            .respond_with(ResponseTemplate::new(404).set_body_json(json!([{
                "errorCode": "NOT_FOUND",
                "message": "List view not found"
            }])))
            .expect(1)
            .mount(&server)
            .await;

        let result = client.ui().list_ui(LIST_VIEW_ID).await;
        assert!(result.is_err());
    }

    // ── list_views ────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_list_views_success() {
        let server = MockServer::start().await;
        let client = make_client(&server).await;

        let response_body = json!({
            "count": 2,
            "currentPageToken": null,
            "nextPageToken": null,
            "previousPageToken": null,
            "listViews": [
                {
                    "apiName": "AllAccounts",
                    "id": LIST_VIEW_ID,
                    "label": "All Accounts",
                    "listViewType": "AllList",
                    "visibility": "Everyone"
                },
                {
                    "apiName": "MyAccounts",
                    "id": LIST_VIEW_ID2,
                    "label": "My Accounts",
                    "listViewType": "MyList",
                    "visibility": "Mine"
                }
            ]
        });

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/ui-api/list-ui/Account"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
            .expect(1)
            .mount(&server)
            .await;

        let result = client.ui().list_views("Account").await.must();

        assert_eq!(result.count, 2);
        assert_eq!(result.list_views.len(), 2);
        assert_eq!(result.list_views[0].api_name, "AllAccounts");
        assert_eq!(result.list_views[1].api_name, "MyAccounts");
        assert_eq!(result.list_views[0].visibility.as_deref(), Some("Everyone"));
    }

    #[tokio::test]
    async fn test_list_views_empty_result() {
        let server = MockServer::start().await;
        let client = make_client(&server).await;

        let response_body = json!({
            "count": 0,
            "currentPageToken": null,
            "nextPageToken": null,
            "previousPageToken": null,
            "listViews": []
        });

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/ui-api/list-ui/NoViews__c"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
            .expect(1)
            .mount(&server)
            .await;

        let result = client.ui().list_views("NoViews__c").await.must();

        assert_eq!(result.count, 0);
        assert!(result.list_views.is_empty());
    }

    // ── list_records ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_list_records_success_no_params() {
        let server = MockServer::start().await;
        let client = make_client(&server).await;

        let response_body = json!({
            "count": 3,
            "currentPageToken": "page1",
            "nextPageToken": null,
            "previousPageToken": null,
            "records": [
                {"id": "001000000000001AAA"},
                {"id": "001000000000002AAA"},
                {"id": "001000000000003AAA"}
            ]
        });

        Mock::given(method("GET"))
            .and(path(format!(
                "/services/data/v60.0/ui-api/list-records/{LIST_VIEW_ID}"
            )))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
            .expect(1)
            .mount(&server)
            .await;

        let result = client
            .ui()
            .list_records(LIST_VIEW_ID, None, None)
            .await
            .must();

        assert_eq!(result.count, 3);
        assert_eq!(result.records.len(), 3);
        assert_eq!(result.current_page_token.as_deref(), Some("page1"));
        assert!(result.next_page_token.is_none());
    }

    #[tokio::test]
    async fn test_list_records_with_page_size() {
        let server = MockServer::start().await;
        let client = make_client(&server).await;

        let response_body = json!({
            "count": 5,
            "currentPageToken": null,
            "nextPageToken": "abc123",
            "previousPageToken": null,
            "records": [
                {"id": "001000000000001AAA"},
                {"id": "001000000000002AAA"},
                {"id": "001000000000003AAA"},
                {"id": "001000000000004AAA"},
                {"id": "001000000000005AAA"}
            ]
        });

        Mock::given(method("GET"))
            .and(path(format!(
                "/services/data/v60.0/ui-api/list-records/{LIST_VIEW_ID}"
            )))
            .and(query_param("pageSize", "5"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
            .expect(1)
            .mount(&server)
            .await;

        let result = client
            .ui()
            .list_records(LIST_VIEW_ID, Some(5), None)
            .await
            .must();

        assert_eq!(result.count, 5);
        assert_eq!(result.next_page_token.as_deref(), Some("abc123"));
    }

    #[tokio::test]
    async fn test_list_records_with_page_size_and_token() {
        let server = MockServer::start().await;
        let client = make_client(&server).await;

        let response_body = json!({
            "count": 2,
            "currentPageToken": "abc123",
            "nextPageToken": "def456",
            "previousPageToken": "prev999",
            "records": [
                {"id": "001000000000006AAA"},
                {"id": "001000000000007AAA"}
            ]
        });

        Mock::given(method("GET"))
            .and(path(format!(
                "/services/data/v60.0/ui-api/list-records/{LIST_VIEW_ID}"
            )))
            .and(query_param("pageSize", "2"))
            .and(query_param("pageToken", "abc123"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
            .expect(1)
            .mount(&server)
            .await;

        let result = client
            .ui()
            .list_records(LIST_VIEW_ID, Some(2), Some("abc123"))
            .await
            .must();

        assert_eq!(result.count, 2);
        assert_eq!(result.current_page_token.as_deref(), Some("abc123"));
        assert_eq!(result.previous_page_token.as_deref(), Some("prev999"));
        assert_eq!(result.next_page_token.as_deref(), Some("def456"));
    }

    #[tokio::test]
    async fn test_list_records_bad_request() {
        let server = MockServer::start().await;
        let client = make_client(&server).await;

        Mock::given(method("GET"))
            .and(path(format!(
                "/services/data/v60.0/ui-api/list-records/{LIST_VIEW_ID}"
            )))
            .respond_with(ResponseTemplate::new(400).set_body_json(json!([{
                "errorCode": "INVALID_PARAMETER_VALUE",
                "message": "Invalid page size"
            }])))
            .expect(1)
            .mount(&server)
            .await;

        let result = client
            .ui()
            .list_records(LIST_VIEW_ID, Some(9999), None)
            .await;
        assert!(result.is_err());
    }

    // ── list_info ─────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_list_info_success() {
        let server = MockServer::start().await;
        let client = make_client(&server).await;

        let response_body = json!({
            "id": LIST_VIEW_ID,
            "apiName": "AllAccounts",
            "label": "All Accounts",
            "listViewType": "AllList",
            "displayColumns": [
                {"fieldApiName": "Name", "label": "Account Name", "sortable": true},
                {"fieldApiName": "Phone", "label": "Phone", "sortable": true}
            ]
        });

        Mock::given(method("GET"))
            .and(path(format!(
                "/services/data/v60.0/ui-api/list-info/{LIST_VIEW_ID}"
            )))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
            .expect(1)
            .mount(&server)
            .await;

        let result = client.ui().list_info(LIST_VIEW_ID).await.must();

        assert_eq!(result.id, LIST_VIEW_ID);
        assert_eq!(result.api_name, "AllAccounts");
        assert_eq!(result.label, "All Accounts");
        assert_eq!(result.list_view_type.as_deref(), Some("AllList"));
        assert_eq!(result.display_columns.len(), 2);
    }

    #[tokio::test]
    async fn test_list_info_not_found() {
        let server = MockServer::start().await;
        let client = make_client(&server).await;

        Mock::given(method("GET"))
            .and(path(format!(
                "/services/data/v60.0/ui-api/list-info/{LIST_VIEW_ID}"
            )))
            .respond_with(ResponseTemplate::new(404).set_body_json(json!([{
                "errorCode": "NOT_FOUND",
                "message": "List view not found"
            }])))
            .expect(1)
            .mount(&server)
            .await;

        let result = client.ui().list_info(LIST_VIEW_ID).await;
        assert!(result.is_err());
    }

    // ── type deserialization unit tests ───────────────────────────────────────

    #[test]
    fn test_list_view_summary_collection_deserialize() {
        let json_str = r#"{
            "count": 3,
            "currentPageToken": "tok1",
            "nextPageToken": "tok2",
            "previousPageToken": null,
            "listViews": [
                {
                    "apiName": "AllAccounts",
                    "id": "00B000000000001AAA",
                    "label": "All Accounts",
                    "listViewType": "AllList",
                    "visibility": "Everyone"
                },
                {
                    "apiName": "MyAccounts",
                    "id": "00B000000000002AAA",
                    "label": "My Accounts",
                    "listViewType": "MyList",
                    "visibility": "Mine"
                },
                {
                    "apiName": "RecentAccounts",
                    "id": "00B000000000003AAA",
                    "label": "Recently Viewed",
                    "listViewType": null,
                    "visibility": null
                }
            ]
        }"#;

        let collection: ListViewSummaryCollection = serde_json::from_str(json_str).unwrap();
        assert_eq!(collection.count, 3);
        assert_eq!(collection.current_page_token.as_deref(), Some("tok1"));
        assert_eq!(collection.next_page_token.as_deref(), Some("tok2"));
        assert!(collection.previous_page_token.is_none());
        assert_eq!(collection.list_views.len(), 3);
        assert_eq!(collection.list_views[0].api_name, "AllAccounts");
        assert_eq!(
            collection.list_views[0].visibility.as_deref(),
            Some("Everyone")
        );
        assert!(collection.list_views[2].list_view_type.is_none());
    }

    #[test]
    fn test_list_records_representation_with_pagination_tokens() {
        let json_str = r#"{
            "count": 50,
            "currentPageToken": "current_tok",
            "nextPageToken": "next_tok",
            "previousPageToken": "prev_tok",
            "records": [
                {"id": "001000000000001AAA", "name": "Acme Corp"},
                {"id": "001000000000002AAA", "name": "Globex"}
            ]
        }"#;

        let records: ListRecordsRepresentation = serde_json::from_str(json_str).unwrap();
        assert_eq!(records.count, 50);
        assert_eq!(records.current_page_token.as_deref(), Some("current_tok"));
        assert_eq!(records.next_page_token.as_deref(), Some("next_tok"));
        assert_eq!(records.previous_page_token.as_deref(), Some("prev_tok"));
        assert_eq!(records.records.len(), 2);
    }
}
