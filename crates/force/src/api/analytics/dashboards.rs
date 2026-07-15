//! Dashboard operations for the Analytics API.
//!
//! Listing, describe, results, refresh, and status polling for dashboards.

use super::AnalyticsHandler;
use super::types::{DashboardListItem, DashboardResults, DashboardStatus};
use crate::error::Result;

impl<A: crate::auth::Authenticator> AnalyticsHandler<A> {
    /// Lists the dashboards visible to the running user.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response cannot be parsed.
    pub async fn list_dashboards(&self) -> Result<Vec<DashboardListItem>> {
        self.get("dashboards", None, "Analytics list_dashboards failed")
            .await
    }

    /// Describes a dashboard's definition and layout metadata.
    ///
    /// Dashboard metadata varies widely by dashboard type (Lightning vs.
    /// classic) and API version, so the raw JSON is returned.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response cannot be parsed.
    pub async fn describe_dashboard(&self, dashboard_id: &str) -> Result<serde_json::Value> {
        self.get(
            &format!("dashboards/{dashboard_id}/describe"),
            None,
            "Analytics describe_dashboard failed",
        )
        .await
    }

    /// Gets a dashboard's last-cached component results and status.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response cannot be parsed.
    pub async fn get_dashboard_results(&self, dashboard_id: &str) -> Result<DashboardResults> {
        self.get(
            &format!("dashboards/{dashboard_id}"),
            None,
            "Analytics get_dashboard_results failed",
        )
        .await
    }

    /// Refreshes a dashboard, recomputing its components asynchronously.
    ///
    /// Returns the refresh status; poll
    /// [`get_dashboard_status`](Self::get_dashboard_status) for completion.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response cannot be parsed.
    pub async fn refresh_dashboard(&self, dashboard_id: &str) -> Result<DashboardStatus> {
        self.put_empty(
            &format!("dashboards/{dashboard_id}"),
            "Analytics refresh_dashboard failed",
        )
        .await
    }

    /// Gets the refresh status of a dashboard and its components.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response cannot be parsed.
    pub async fn get_dashboard_status(&self, dashboard_id: &str) -> Result<DashboardStatus> {
        self.get(
            &format!("dashboards/{dashboard_id}/status"),
            None,
            "Analytics get_dashboard_status failed",
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use crate::client::{ForceClient, builder};
    use crate::test_utils::mock_auth::MockAuthenticator;
    use crate::test_utils::must::Must;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn setup() -> (MockServer, ForceClient<MockAuthenticator>) {
        let server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &server.uri());
        let client = builder().authenticate(auth).build().await.must();
        (server, client)
    }

    #[tokio::test]
    async fn test_list_dashboards() {
        let (server, client) = setup().await;

        Mock::given(method("GET"))
            .and(path("/services/data/v67.0/analytics/dashboards"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {
                    "id": "01Z3000000ABCDE",
                    "name": "Sales Overview",
                    "url": "/services/data/v67.0/analytics/dashboards/01Z3000000ABCDE",
                    "type": "Dashboard"
                }
            ])))
            .mount(&server)
            .await;

        let dashboards = client.analytics().list_dashboards().await.must();
        assert_eq!(dashboards.len(), 1);
        assert_eq!(dashboards[0].id.as_deref(), Some("01Z3000000ABCDE"));
        assert_eq!(dashboards[0].name.as_deref(), Some("Sales Overview"));
        assert_eq!(dashboards[0].dashboard_type.as_deref(), Some("Dashboard"));
    }

    #[tokio::test]
    async fn test_describe_dashboard() {
        let (server, client) = setup().await;

        Mock::given(method("GET"))
            .and(path(
                "/services/data/v67.0/analytics/dashboards/01Z/describe",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "01Z",
                "name": "Sales Overview",
                "developerName": "Sales_Overview",
                "components": [
                    { "id": "01a", "reportId": "00O", "title": "Pipeline", "componentType": "Bar" }
                ]
            })))
            .mount(&server)
            .await;

        let describe = client.analytics().describe_dashboard("01Z").await.must();
        assert_eq!(describe["id"], "01Z");
        assert_eq!(describe["components"][0]["componentType"], "Bar");
    }

    #[tokio::test]
    async fn test_get_dashboard_results() {
        let (server, client) = setup().await;

        Mock::given(method("GET"))
            .and(path("/services/data/v67.0/analytics/dashboards/01Z"))
            .and(header("Authorization", "Bearer test_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "01Z",
                "name": "Sales Overview",
                "componentData": [
                    {
                        "componentId": "01a",
                        "reportResult": { "hasDetailRows": false, "factMap": {} },
                        "status": { "componentStatus": "Success" }
                    }
                ]
            })))
            .mount(&server)
            .await;

        let results = client.analytics().get_dashboard_results("01Z").await.must();
        assert_eq!(results.id.as_deref(), Some("01Z"));
        assert_eq!(results.component_data.len(), 1);
        assert_eq!(results.component_data[0]["componentId"], "01a");
    }

    #[tokio::test]
    async fn test_refresh_dashboard() {
        let (server, client) = setup().await;

        Mock::given(method("PUT"))
            .and(path("/services/data/v67.0/analytics/dashboards/01Z"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "dashboardId": "01Z",
                "requestDate": "2026-07-12T18:00:05Z",
                "componentStatus": [
                    { "componentId": "01a", "status": "Running", "errorMessage": null }
                ]
            })))
            .expect(1)
            .mount(&server)
            .await;

        let status = client.analytics().refresh_dashboard("01Z").await.must();
        assert_eq!(status.dashboard_id.as_deref(), Some("01Z"));
        assert_eq!(status.component_status.len(), 1);
        assert_eq!(
            status.component_status[0].status.as_deref(),
            Some("Running")
        );
    }

    #[tokio::test]
    async fn test_get_dashboard_status() {
        let (server, client) = setup().await;

        Mock::given(method("GET"))
            .and(path("/services/data/v67.0/analytics/dashboards/01Z/status"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "dashboardId": "01Z",
                "requestDate": "2026-07-12T18:00:05Z",
                "componentStatus": [
                    { "componentId": "01a", "status": "Success", "errorMessage": null }
                ]
            })))
            .mount(&server)
            .await;

        let status = client.analytics().get_dashboard_status("01Z").await.must();
        assert_eq!(status.dashboard_id.as_deref(), Some("01Z"));
        assert_eq!(
            status.component_status[0].status.as_deref(),
            Some("Success")
        );
    }

    #[tokio::test]
    async fn test_get_dashboard_results_server_error() {
        let (server, client) = setup().await;

        Mock::given(method("GET"))
            .and(path("/services/data/v67.0/analytics/dashboards/01Zbad"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let result = client.analytics().get_dashboard_results("01Zbad").await;
        assert!(result.is_err());
    }
}
