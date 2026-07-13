//! Salesforce Reports & Dashboards (Analytics) REST API handler.
//!
//! The Analytics API (`/services/data/vXX.X/analytics/`) runs reports and
//! dashboards and returns their results and metadata. Reports can be run
//! synchronously or asynchronously (via instances), described, listed, and
//! executed ad-hoc with dynamic metadata overrides; dashboards can be listed,
//! described, read, refreshed, and polled for status.
//!
//! # Feature Flag
//!
//! This module requires the `analytics` feature flag:
//! ```toml
//! [dependencies]
//! force = { version = "...", features = ["analytics"] }
//! ```
//!
//! # Usage
//!
//! ```ignore
//! let client = builder().authenticate(auth).build().await?;
//! let analytics = client.analytics();
//!
//! // Run a report synchronously with detail rows.
//! let results = analytics.run_report("00O3000000B5Yn2", true).await?;
//! let grand_total = &results.fact_map["T!T"].aggregates[0];
//! println!("total = {}", grand_total.label);
//!
//! // Read dashboard results.
//! let dash = analytics.get_dashboard_results("01Z3000000ABCDE").await?;
//! ```

#![allow(clippy::doc_markdown)]

pub(crate) mod dashboards;
pub(crate) mod reports;
pub(crate) mod types;

// Re-export the public types at module level.
pub use types::{
    ComponentStatus, DashboardListItem, DashboardResults, DashboardStatus, DataCell, FactMapEntry,
    FilterOperator, Grouping, GroupingInfo, GroupingsContainer, InstanceStatus, ReportAttributes,
    ReportDescribe, ReportExtendedMetadata, ReportFilter, ReportFormat, ReportInstance,
    ReportListItem, ReportMetadata, ReportMetadataRequest, ReportResults, ReportRow,
    ReportTypeCategory, ReportTypeInfo, ReportTypeRef, StandardDateFilter, SummaryValue,
};

use crate::error::Result;
use std::sync::Arc;

/// Reports & Dashboards (Analytics) API handler.
///
/// Provides access to the Salesforce Reports and Dashboards REST API under the
/// `/services/data/vXX.X/analytics/` URL prefix. Report operations and
/// dashboard operations are implemented as methods on this handler.
///
/// Obtain a handler from [`ForceClient::analytics`](crate::client::ForceClient::analytics).
#[derive(Debug)]
pub struct AnalyticsHandler<A: crate::auth::Authenticator> {
    /// Reference to the shared session state.
    inner: Arc<crate::session::Session<A>>,
}

impl<A: crate::auth::Authenticator> Clone for AnalyticsHandler<A> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<A: crate::auth::Authenticator> AnalyticsHandler<A> {
    /// Creates a new Analytics handler wrapping the given session.
    #[must_use]
    pub(crate) fn new(inner: Arc<crate::session::Session<A>>) -> Self {
        Self { inner }
    }

    /// Resolves an Analytics API path to a full URL.
    ///
    /// Constructs: `{instance_url}/services/data/{version}/analytics/{path}`
    pub(crate) async fn resolve_analytics_url(&self, path: &str) -> Result<String> {
        let clean = path.trim_start_matches('/');
        if clean.is_empty() {
            self.inner.resolve_url("analytics").await
        } else {
            self.inner.resolve_url(&format!("analytics/{clean}")).await
        }
    }

    /// Helper: GET an Analytics API path and deserialize the JSON response.
    pub(crate) async fn get<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        query: Option<&[(&str, &str)]>,
        error_msg: &str,
    ) -> Result<T> {
        let url = self.resolve_analytics_url(path).await?;
        let mut req = self.inner.get(&url);
        if let Some(params) = query {
            req = req.query(params);
        }
        let request = req.build().map_err(crate::error::HttpError::from)?;
        self.inner.send_request_and_decode(request, error_msg).await
    }

    /// Helper: POST to an Analytics API path and deserialize the JSON response.
    pub(crate) async fn post<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        query: Option<&[(&str, &str)]>,
        body: &(impl serde::Serialize + Sync),
        error_msg: &str,
    ) -> Result<T> {
        let url = self.resolve_analytics_url(path).await?;
        let mut req = self.inner.post(&url).json(body);
        if let Some(params) = query {
            req = req.query(params);
        }
        let request = req.build().map_err(crate::error::HttpError::from)?;
        self.inner.send_request_and_decode(request, error_msg).await
    }

    /// Helper: POST to an Analytics API path with no request body and
    /// deserialize the JSON response.
    pub(crate) async fn post_empty<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        query: Option<&[(&str, &str)]>,
        error_msg: &str,
    ) -> Result<T> {
        let url = self.resolve_analytics_url(path).await?;
        let mut req = self.inner.post(&url);
        if let Some(params) = query {
            req = req.query(params);
        }
        let request = req.build().map_err(crate::error::HttpError::from)?;
        self.inner.send_request_and_decode(request, error_msg).await
    }

    /// Helper: PUT to an Analytics API path with no request body and
    /// deserialize the JSON response.
    pub(crate) async fn put_empty<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        error_msg: &str,
    ) -> Result<T> {
        let url = self.resolve_analytics_url(path).await?;
        let request = self
            .inner
            .put(&url)
            .build()
            .map_err(crate::error::HttpError::from)?;
        self.inner.send_request_and_decode(request, error_msg).await
    }

    /// Helper: DELETE an Analytics API path; expect an empty success response.
    pub(crate) async fn delete_empty(&self, path: &str, error_msg: &str) -> Result<()> {
        let url = self.resolve_analytics_url(path).await?;
        let request = self
            .inner
            .delete(&url)
            .build()
            .map_err(crate::error::HttpError::from)?;
        self.inner
            .execute_and_check_success(request, error_msg)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::client::{ForceClient, builder};
    use crate::test_utils::mock_auth::MockAuthenticator;
    use crate::test_utils::must::{Must, MustMsg};

    async fn test_client() -> ForceClient<MockAuthenticator> {
        let auth = MockAuthenticator::new("test_token", "https://test.salesforce.com");
        builder()
            .authenticate(auth)
            .build()
            .await
            .must_msg("failed to create test client")
    }

    #[tokio::test]
    async fn test_analytics_handler_construction() {
        let client = test_client().await;
        let _handler = client.analytics();
    }

    #[tokio::test]
    async fn test_analytics_handler_is_cloneable() {
        let client = test_client().await;
        let h1 = client.analytics();
        let h2 = h1.clone();
        let url1 = h1.resolve_analytics_url("").await.must();
        let url2 = h2.resolve_analytics_url("").await.must();
        assert_eq!(url1, url2);
    }

    #[tokio::test]
    async fn test_resolve_analytics_url_base() {
        let client = test_client().await;
        let handler = client.analytics();
        let url = handler.resolve_analytics_url("").await.must();
        assert!(url.contains("/services/data/"));
        assert!(url.ends_with("analytics"));
    }

    #[tokio::test]
    async fn test_resolve_analytics_url_with_path() {
        let client = test_client().await;
        let handler = client.analytics();
        let url = handler
            .resolve_analytics_url("reports/00O3000000B5Yn2")
            .await
            .must();
        assert!(url.contains("analytics/reports/00O3000000B5Yn2"));
    }

    #[tokio::test]
    async fn test_resolve_analytics_url_leading_slash() {
        let client = test_client().await;
        let handler = client.analytics();
        let url1 = handler.resolve_analytics_url("dashboards").await.must();
        let url2 = handler.resolve_analytics_url("/dashboards").await.must();
        assert_eq!(url1, url2);
    }

    #[tokio::test]
    async fn test_analytics_url_includes_api_version() {
        let client = test_client().await;
        let handler = client.analytics();
        let url = handler.resolve_analytics_url("reports").await.must();
        assert!(url.contains("v67.0"), "URL should contain API version");
    }

    #[tokio::test]
    async fn test_analytics_handler_debug() {
        let client = test_client().await;
        let handler = client.analytics();
        let debug = format!("{handler:?}");
        assert!(debug.contains("AnalyticsHandler"));
    }
}
