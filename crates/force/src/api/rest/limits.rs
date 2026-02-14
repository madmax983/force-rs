//! Organization Limits API.
//!
//! This module provides types and methods for retrieving Salesforce org limits,
//! including API usage, storage capacity, and other resource constraints.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Response from the Organization Limits API endpoint.
///
/// Contains information about various organizational limits including:
/// - Daily API request limits
/// - Data storage limits
/// - File storage limits
/// - Streaming API limits
/// - And many other resource constraints
///
/// # Examples
///
/// ```ignore
/// let limits = client.rest().limits().await?;
/// let api_limit = &limits.daily_api_requests;
/// println!("API calls used: {}/{}", api_limit.used, api_limit.max);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct OrgLimits {
    /// Daily API request limit and usage.
    pub daily_api_requests: LimitInfo,

    /// Daily asynchronous Apex executions limit.
    pub daily_async_apex_executions: LimitInfo,

    /// Daily batch Apex executions limit.
    pub daily_batch_apex_executions: LimitInfo,

    /// Daily durable generic streaming API events limit.
    pub daily_durable_generic_streaming_api_events: LimitInfo,

    /// Daily durable streaming API events limit.
    pub daily_durable_streaming_api_events: LimitInfo,

    /// Daily generic streaming API events limit.
    pub daily_generic_streaming_api_events: LimitInfo,

    /// Daily streaming API events limit.
    pub daily_streaming_api_events: LimitInfo,

    /// Daily workflow emails limit.
    pub daily_workflow_emails: LimitInfo,

    /// Data storage (MB).
    #[serde(rename = "DataStorageMB")]
    pub data_storage_mb: LimitInfo,

    /// File storage (MB).
    #[serde(rename = "FileStorageMB")]
    pub file_storage_mb: LimitInfo,

    /// Hourly asynchronous report runs limit.
    pub hourly_async_report_runs: LimitInfo,

    /// Hourly dashboard refreshes limit.
    pub hourly_dashboard_refreshes: LimitInfo,

    /// Hourly dashboard results limit.
    pub hourly_dashboard_results: LimitInfo,

    /// Hourly dashboard status limit.
    pub hourly_dashboard_statuses: LimitInfo,

    /// Hourly long-term ID mapping limit.
    #[serde(rename = "HourlyLongTermIdMapping")]
    pub hourly_long_term_id_mapping: LimitInfo,

    /// Hourly managed content public requests limit.
    pub hourly_managed_content_public_requests: LimitInfo,

    /// Hourly OData callout limit.
    #[serde(rename = "HourlyODataCallout")]
    pub hourly_o_data_callout: LimitInfo,

    /// Hourly short-term ID mapping limit.
    #[serde(rename = "HourlyShortTermIdMapping")]
    pub hourly_short_term_id_mapping: LimitInfo,

    /// Hourly time-based workflow limit.
    pub hourly_time_based_workflow: LimitInfo,

    /// Mass email limit.
    pub mass_email: LimitInfo,

    /// Single email limit.
    pub single_email: LimitInfo,

    /// Additional limits returned by Salesforce.
    ///
    /// Salesforce may add new limits over time. This field captures any
    /// limits not explicitly defined in the struct above.
    #[serde(flatten)]
    pub additional_limits: HashMap<String, LimitInfo>,
}

/// Information about a specific limit including current usage and maximum allowed.
///
/// # Examples
///
/// ```
/// use force::api::rest::limits::LimitInfo;
///
/// let limit = LimitInfo {
///     max: 15000,
///     remaining: 14850,
///     used: Some(150),
/// };
///
/// assert_eq!(limit.percentage_used(), 1.0);
/// assert!(!limit.is_at_limit());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LimitInfo {
    /// Maximum allowed for this limit.
    pub max: i64,

    /// Remaining capacity before hitting the limit.
    pub remaining: i64,

    /// Amount currently used (may not be present for all limits).
    ///
    /// When present, `used + remaining` should equal `max`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub used: Option<i64>,
}

impl LimitInfo {
    /// Creates a new limit info with the given values.
    ///
    /// # Arguments
    ///
    /// * `max` - Maximum allowed for this limit
    /// * `remaining` - Remaining capacity
    /// * `used` - Optional amount currently used
    #[must_use]
    pub const fn new(max: i64, remaining: i64, used: Option<i64>) -> Self {
        Self {
            max,
            remaining,
            used,
        }
    }

    /// Returns the percentage of the limit that has been used (0.0 to 100.0).
    ///
    /// # Examples
    ///
    /// ```
    /// use force::api::rest::limits::LimitInfo;
    ///
    /// let limit = LimitInfo::new(1000, 250, Some(750));
    /// assert_eq!(limit.percentage_used(), 75.0);
    /// ```
    #[must_use]
    pub fn percentage_used(&self) -> f64 {
        if self.max == 0 {
            return 0.0;
        }
        let used = self.used.unwrap_or(self.max - self.remaining);
        (used as f64 / self.max as f64) * 100.0
    }

    /// Returns true if the limit has been reached.
    #[must_use]
    pub const fn is_at_limit(&self) -> bool {
        self.remaining == 0
    }

    /// Returns true if usage is above the given percentage threshold.
    ///
    /// # Arguments
    ///
    /// * `threshold` - Percentage threshold (0.0 to 100.0)
    ///
    /// # Examples
    ///
    /// ```
    /// use force::api::rest::limits::LimitInfo;
    ///
    /// let limit = LimitInfo::new(1000, 100, Some(900));
    /// assert!(limit.is_above_threshold(80.0));
    /// assert!(!limit.is_above_threshold(95.0));
    /// ```
    #[must_use]
    pub fn is_above_threshold(&self, threshold: f64) -> bool {
        self.percentage_used() > threshold
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Must;

    // RED PHASE - Write failing tests first

    #[test]
    fn test_limit_info_new() {
        let limit = LimitInfo::new(1000, 250, Some(750));

        assert_eq!(limit.max, 1000);
        assert_eq!(limit.remaining, 250);
        assert_eq!(limit.used, Some(750));
    }

    #[test]
    fn test_limit_info_percentage_used() {
        let limit = LimitInfo::new(1000, 250, Some(750));
        assert!((limit.percentage_used() - 75.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_limit_info_percentage_used_without_explicit_used() {
        let limit = LimitInfo::new(1000, 300, None);
        // Should calculate: max - remaining = 1000 - 300 = 700
        assert!((limit.percentage_used() - 70.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_limit_info_percentage_used_zero_max() {
        let limit = LimitInfo::new(0, 0, Some(0));
        assert!((limit.percentage_used() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_limit_info_is_at_limit() {
        let at_limit = LimitInfo::new(1000, 0, Some(1000));
        assert!(at_limit.is_at_limit());

        let not_at_limit = LimitInfo::new(1000, 100, Some(900));
        assert!(!not_at_limit.is_at_limit());
    }

    #[test]
    fn test_limit_info_is_above_threshold() {
        let limit = LimitInfo::new(1000, 100, Some(900));

        assert!(limit.is_above_threshold(80.0));
        assert!(limit.is_above_threshold(89.0));
        assert!(!limit.is_above_threshold(90.0));
        assert!(!limit.is_above_threshold(95.0));
    }

    #[test]
    fn test_limit_info_serialize() {
        let limit = LimitInfo::new(15000, 14850, Some(150));
        let json = serde_json::to_string(&limit).must();

        assert!(json.contains("\"Max\":15000"));
        assert!(json.contains("\"Remaining\":14850"));
        assert!(json.contains("\"Used\":150"));
    }

    #[test]
    fn test_limit_info_deserialize() {
        let json = r#"{
            "Max": 15000,
            "Remaining": 14850,
            "Used": 150
        }"#;

        let limit: LimitInfo = serde_json::from_str(json).must();
        assert_eq!(limit.max, 15000);
        assert_eq!(limit.remaining, 14850);
        assert_eq!(limit.used, Some(150));
    }

    #[test]
    fn test_limit_info_deserialize_without_used() {
        let json = r#"{
            "Max": 2000000,
            "Remaining": 1999000
        }"#;

        let limit: LimitInfo = serde_json::from_str(json).must();
        assert_eq!(limit.max, 2_000_000);
        assert_eq!(limit.remaining, 1_999_000);
        assert_eq!(limit.used, None);
    }

    #[test]
    fn test_org_limits_deserialize() {
        let json = r#"{
            "DailyApiRequests": {
                "Max": 15000,
                "Remaining": 14850,
                "Used": 150
            },
            "DailyAsyncApexExecutions": {
                "Max": 250000,
                "Remaining": 250000
            },
            "DailyBatchApexExecutions": {
                "Max": 250000,
                "Remaining": 250000
            },
            "DailyDurableGenericStreamingApiEvents": {
                "Max": 10000,
                "Remaining": 10000
            },
            "DailyDurableStreamingApiEvents": {
                "Max": 10000,
                "Remaining": 10000
            },
            "DailyGenericStreamingApiEvents": {
                "Max": 10000,
                "Remaining": 10000
            },
            "DailyStreamingApiEvents": {
                "Max": 10000,
                "Remaining": 10000
            },
            "DailyWorkflowEmails": {
                "Max": 1000,
                "Remaining": 1000
            },
            "DataStorageMB": {
                "Max": 5120,
                "Remaining": 4800
            },
            "FileStorageMB": {
                "Max": 20480,
                "Remaining": 20000
            },
            "HourlyAsyncReportRuns": {
                "Max": 1200,
                "Remaining": 1200
            },
            "HourlyDashboardRefreshes": {
                "Max": 200,
                "Remaining": 200
            },
            "HourlyDashboardResults": {
                "Max": 5000,
                "Remaining": 5000
            },
            "HourlyDashboardStatuses": {
                "Max": 999999999,
                "Remaining": 999999999
            },
            "HourlyLongTermIdMapping": {
                "Max": 100000,
                "Remaining": 100000
            },
            "HourlyManagedContentPublicRequests": {
                "Max": 50000,
                "Remaining": 50000
            },
            "HourlyODataCallout": {
                "Max": 10000,
                "Remaining": 10000
            },
            "HourlyShortTermIdMapping": {
                "Max": 100000,
                "Remaining": 100000
            },
            "HourlyTimeBasedWorkflow": {
                "Max": 1000,
                "Remaining": 1000
            },
            "MassEmail": {
                "Max": 10,
                "Remaining": 10
            },
            "SingleEmail": {
                "Max": 15,
                "Remaining": 15
            }
        }"#;

        let limits: OrgLimits = serde_json::from_str(json).must();
        assert_eq!(limits.daily_api_requests.max, 15000);
        assert_eq!(limits.daily_api_requests.remaining, 14850);
        assert_eq!(limits.daily_api_requests.used, Some(150));
        assert_eq!(limits.data_storage_mb.max, 5120);
        assert_eq!(limits.file_storage_mb.max, 20480);
    }

    #[test]
    fn test_org_limits_with_additional_limits() {
        let json = r#"{
            "DailyApiRequests": {"Max": 15000, "Remaining": 14850},
            "DailyAsyncApexExecutions": {"Max": 250000, "Remaining": 250000},
            "DailyBatchApexExecutions": {"Max": 250000, "Remaining": 250000},
            "DailyDurableGenericStreamingApiEvents": {"Max": 10000, "Remaining": 10000},
            "DailyDurableStreamingApiEvents": {"Max": 10000, "Remaining": 10000},
            "DailyGenericStreamingApiEvents": {"Max": 10000, "Remaining": 10000},
            "DailyStreamingApiEvents": {"Max": 10000, "Remaining": 10000},
            "DailyWorkflowEmails": {"Max": 1000, "Remaining": 1000},
            "DataStorageMB": {"Max": 5120, "Remaining": 4800},
            "FileStorageMB": {"Max": 20480, "Remaining": 20000},
            "HourlyAsyncReportRuns": {"Max": 1200, "Remaining": 1200},
            "HourlyDashboardRefreshes": {"Max": 200, "Remaining": 200},
            "HourlyDashboardResults": {"Max": 5000, "Remaining": 5000},
            "HourlyDashboardStatuses": {"Max": 999999999, "Remaining": 999999999},
            "HourlyLongTermIdMapping": {"Max": 100000, "Remaining": 100000},
            "HourlyManagedContentPublicRequests": {"Max": 50000, "Remaining": 50000},
            "HourlyODataCallout": {"Max": 10000, "Remaining": 10000},
            "HourlyShortTermIdMapping": {"Max": 100000, "Remaining": 100000},
            "HourlyTimeBasedWorkflow": {"Max": 1000, "Remaining": 1000},
            "MassEmail": {"Max": 10, "Remaining": 10},
            "SingleEmail": {"Max": 15, "Remaining": 15},
            "FutureLimit": {"Max": 999, "Remaining": 888}
        }"#;

        let limits: OrgLimits = serde_json::from_str(json).must();
        assert!(limits.additional_limits.contains_key("FutureLimit"));
        assert_eq!(limits.additional_limits["FutureLimit"].max, 999);
    }

    #[test]
    fn test_org_limits_roundtrip() {
        let original = OrgLimits {
            daily_api_requests: LimitInfo::new(15000, 14850, Some(150)),
            daily_async_apex_executions: LimitInfo::new(250_000, 250_000, None),
            daily_batch_apex_executions: LimitInfo::new(250_000, 250_000, None),
            daily_durable_generic_streaming_api_events: LimitInfo::new(10000, 10000, None),
            daily_durable_streaming_api_events: LimitInfo::new(10000, 10000, None),
            daily_generic_streaming_api_events: LimitInfo::new(10000, 10000, None),
            daily_streaming_api_events: LimitInfo::new(10000, 10000, None),
            daily_workflow_emails: LimitInfo::new(1000, 1000, None),
            data_storage_mb: LimitInfo::new(5120, 4800, None),
            file_storage_mb: LimitInfo::new(20480, 20000, None),
            hourly_async_report_runs: LimitInfo::new(1200, 1200, None),
            hourly_dashboard_refreshes: LimitInfo::new(200, 200, None),
            hourly_dashboard_results: LimitInfo::new(5000, 5000, None),
            hourly_dashboard_statuses: LimitInfo::new(999_999_999, 999_999_999, None),
            hourly_long_term_id_mapping: LimitInfo::new(100_000, 100_000, None),
            hourly_managed_content_public_requests: LimitInfo::new(50000, 50000, None),
            hourly_o_data_callout: LimitInfo::new(10000, 10000, None),
            hourly_short_term_id_mapping: LimitInfo::new(100_000, 100_000, None),
            hourly_time_based_workflow: LimitInfo::new(1000, 1000, None),
            mass_email: LimitInfo::new(10, 10, None),
            single_email: LimitInfo::new(15, 15, None),
            additional_limits: HashMap::new(),
        };

        let json = serde_json::to_string(&original).must();
        let deserialized: OrgLimits = serde_json::from_str(&json).must();

        assert_eq!(original, deserialized);
    }
}

// Integration tests with wiremock
#[cfg(all(test, feature = "mock"))]
mod integration_tests {
    use crate::auth::{AccessToken, Authenticator, TokenResponse};
    use crate::client::builder;
    use crate::config::ClientConfigBuilder;
    use crate::error::Result;
    use crate::test_support::{Must, MustMsg};
    use async_trait::async_trait;
    use wiremock::matchers::{bearer_token, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    // Mock authenticator for integration tests
    #[derive(Debug, Clone)]
    struct MockAuthenticator {
        token: String,
        instance_url: String,
    }

    impl MockAuthenticator {
        fn new(token: &str, instance_url: &str) -> Self {
            Self {
                token: token.to_string(),
                instance_url: instance_url.to_string(),
            }
        }
    }

    #[async_trait]
    impl Authenticator for MockAuthenticator {
        async fn authenticate(&self) -> Result<AccessToken> {
            Ok(AccessToken::from_response(TokenResponse {
                access_token: self.token.clone(),
                instance_url: self.instance_url.clone(),
                token_type: "Bearer".to_string(),
                issued_at: "1704067200000".to_string(),
                signature: "test_sig".to_string(),
                expires_in: Some(7200),
                refresh_token: None,
            }))
        }

        async fn refresh(&self) -> Result<AccessToken> {
            self.authenticate().await
        }
    }

    fn sample_limits_response() -> serde_json::Value {
        serde_json::json!({
            "DailyApiRequests": {
                "Max": 15000,
                "Remaining": 14850,
                "Used": 150
            },
            "DailyAsyncApexExecutions": {
                "Max": 250_000,
                "Remaining": 250_000
            },
            "DailyBatchApexExecutions": {
                "Max": 250_000,
                "Remaining": 250_000
            },
            "DailyDurableGenericStreamingApiEvents": {
                "Max": 10000,
                "Remaining": 10000
            },
            "DailyDurableStreamingApiEvents": {
                "Max": 10000,
                "Remaining": 10000
            },
            "DailyGenericStreamingApiEvents": {
                "Max": 10000,
                "Remaining": 10000
            },
            "DailyStreamingApiEvents": {
                "Max": 10000,
                "Remaining": 10000
            },
            "DailyWorkflowEmails": {
                "Max": 1000,
                "Remaining": 995,
                "Used": 5
            },
            "DataStorageMB": {
                "Max": 5120,
                "Remaining": 4800
            },
            "FileStorageMB": {
                "Max": 20480,
                "Remaining": 20000
            },
            "HourlyAsyncReportRuns": {
                "Max": 1200,
                "Remaining": 1200
            },
            "HourlyDashboardRefreshes": {
                "Max": 200,
                "Remaining": 200
            },
            "HourlyDashboardResults": {
                "Max": 5000,
                "Remaining": 5000
            },
            "HourlyDashboardStatuses": {
                "Max": 999_999_999,
                "Remaining": 999_999_999
            },
            "HourlyLongTermIdMapping": {
                "Max": 100_000,
                "Remaining": 100_000
            },
            "HourlyManagedContentPublicRequests": {
                "Max": 50000,
                "Remaining": 50000
            },
            "HourlyODataCallout": {
                "Max": 10000,
                "Remaining": 10000
            },
            "HourlyShortTermIdMapping": {
                "Max": 100_000,
                "Remaining": 100_000
            },
            "HourlyTimeBasedWorkflow": {
                "Max": 1000,
                "Remaining": 1000
            },
            "MassEmail": {
                "Max": 10,
                "Remaining": 10
            },
            "SingleEmail": {
                "Max": 15,
                "Remaining": 15
            }
        })
    }

    #[tokio::test]
    async fn test_limits_success() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/limits"))
            .and(bearer_token("test_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(sample_limits_response()))
            .mount(&mock_server)
            .await;

        let client = builder()
            .authenticate(auth)
            .build()
            .await
            .must_msg("Failed to build client");

        let limits = client
            .rest()
            .limits()
            .await
            .must_msg("Failed to get limits");

        assert_eq!(limits.daily_api_requests.max, 15000);
        assert_eq!(limits.daily_api_requests.remaining, 14850);
        assert_eq!(limits.daily_api_requests.used, Some(150));
        assert_eq!(limits.daily_workflow_emails.used, Some(5));
        assert_eq!(limits.data_storage_mb.max, 5120);
    }

    #[tokio::test]
    async fn test_limits_with_custom_api_version() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("custom_token", &mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/services/data/v59.0/limits"))
            .and(bearer_token("custom_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(sample_limits_response()))
            .mount(&mock_server)
            .await;

        let config = ClientConfigBuilder::new().api_version("v59.0").build();
        let client = builder()
            .authenticate(auth)
            .config(config)
            .build()
            .await
            .must_msg("Failed to build client");

        let limits = client
            .rest()
            .limits()
            .await
            .must_msg("Failed to get limits");
        assert_eq!(limits.daily_api_requests.max, 15000);
    }

    #[tokio::test]
    async fn test_limits_unauthorized() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("invalid_token", &mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/limits"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&mock_server)
            .await;

        let client = builder()
            .authenticate(auth)
            .build()
            .await
            .must_msg("Failed to build client");

        let result = client.rest().limits().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_limits_server_error() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/limits"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock_server)
            .await;

        let client = builder()
            .authenticate(auth)
            .build()
            .await
            .must_msg("Failed to build client");

        let result = client.rest().limits().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_limits_correct_headers() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("header_test_token", &mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/limits"))
            .and(header("Authorization", "Bearer header_test_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(sample_limits_response()))
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = builder()
            .authenticate(auth)
            .build()
            .await
            .must_msg("Failed to build client");

        client
            .rest()
            .limits()
            .await
            .must_msg("Failed to get limits");
        // Mock will verify the headers were correct
    }

    #[tokio::test]
    async fn test_limits_at_limit() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());

        let at_limit_response = serde_json::json!({
            "DailyApiRequests": {
                "Max": 15000,
                "Remaining": 0,
                "Used": 15000
            },
            "DailyAsyncApexExecutions": {"Max": 250_000, "Remaining": 250_000},
            "DailyBatchApexExecutions": {"Max": 250_000, "Remaining": 250_000},
            "DailyDurableGenericStreamingApiEvents": {"Max": 10000, "Remaining": 10000},
            "DailyDurableStreamingApiEvents": {"Max": 10000, "Remaining": 10000},
            "DailyGenericStreamingApiEvents": {"Max": 10000, "Remaining": 10000},
            "DailyStreamingApiEvents": {"Max": 10000, "Remaining": 10000},
            "DailyWorkflowEmails": {"Max": 1000, "Remaining": 1000},
            "DataStorageMB": {"Max": 5120, "Remaining": 4800},
            "FileStorageMB": {"Max": 20480, "Remaining": 20000},
            "HourlyAsyncReportRuns": {"Max": 1200, "Remaining": 1200},
            "HourlyDashboardRefreshes": {"Max": 200, "Remaining": 200},
            "HourlyDashboardResults": {"Max": 5000, "Remaining": 5000},
            "HourlyDashboardStatuses": {"Max": 999_999_999, "Remaining": 999_999_999},
            "HourlyLongTermIdMapping": {"Max": 100_000, "Remaining": 100_000},
            "HourlyManagedContentPublicRequests": {"Max": 50000, "Remaining": 50000},
            "HourlyODataCallout": {"Max": 10000, "Remaining": 10000},
            "HourlyShortTermIdMapping": {"Max": 100_000, "Remaining": 100_000},
            "HourlyTimeBasedWorkflow": {"Max": 1000, "Remaining": 1000},
            "MassEmail": {"Max": 10, "Remaining": 10},
            "SingleEmail": {"Max": 15, "Remaining": 15}
        });

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/limits"))
            .respond_with(ResponseTemplate::new(200).set_body_json(at_limit_response))
            .mount(&mock_server)
            .await;

        let client = builder()
            .authenticate(auth)
            .build()
            .await
            .must_msg("Failed to build client");

        let limits = client
            .rest()
            .limits()
            .await
            .must_msg("Failed to get limits");
        assert!(limits.daily_api_requests.is_at_limit());
        assert!((limits.daily_api_requests.percentage_used() - 100.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_limits_with_additional_unknown_limits() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());

        let mut response = sample_limits_response();
        response.as_object_mut().must().insert(
            "FutureNewLimit".to_string(),
            serde_json::json!({"Max": 5000, "Remaining": 4500}),
        );

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/limits"))
            .respond_with(ResponseTemplate::new(200).set_body_json(response))
            .mount(&mock_server)
            .await;

        let client = builder()
            .authenticate(auth)
            .build()
            .await
            .must_msg("Failed to build client");

        let limits = client
            .rest()
            .limits()
            .await
            .must_msg("Failed to get limits");
        assert!(limits.additional_limits.contains_key("FutureNewLimit"));
        assert_eq!(limits.additional_limits["FutureNewLimit"].max, 5000);
    }

    #[tokio::test]
    async fn test_limits_threshold_warnings() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());

        let high_usage_response = serde_json::json!({
            "DailyApiRequests": {
                "Max": 15000,
                "Remaining": 1500,
                "Used": 13500
            },
            "DailyAsyncApexExecutions": {"Max": 250_000, "Remaining": 250_000},
            "DailyBatchApexExecutions": {"Max": 250_000, "Remaining": 250_000},
            "DailyDurableGenericStreamingApiEvents": {"Max": 10000, "Remaining": 10000},
            "DailyDurableStreamingApiEvents": {"Max": 10000, "Remaining": 10000},
            "DailyGenericStreamingApiEvents": {"Max": 10000, "Remaining": 10000},
            "DailyStreamingApiEvents": {"Max": 10000, "Remaining": 10000},
            "DailyWorkflowEmails": {"Max": 1000, "Remaining": 1000},
            "DataStorageMB": {"Max": 5120, "Remaining": 4800},
            "FileStorageMB": {"Max": 20480, "Remaining": 20000},
            "HourlyAsyncReportRuns": {"Max": 1200, "Remaining": 1200},
            "HourlyDashboardRefreshes": {"Max": 200, "Remaining": 200},
            "HourlyDashboardResults": {"Max": 5000, "Remaining": 5000},
            "HourlyDashboardStatuses": {"Max": 999_999_999, "Remaining": 999_999_999},
            "HourlyLongTermIdMapping": {"Max": 100_000, "Remaining": 100_000},
            "HourlyManagedContentPublicRequests": {"Max": 50000, "Remaining": 50000},
            "HourlyODataCallout": {"Max": 10000, "Remaining": 10000},
            "HourlyShortTermIdMapping": {"Max": 100_000, "Remaining": 100_000},
            "HourlyTimeBasedWorkflow": {"Max": 1000, "Remaining": 1000},
            "MassEmail": {"Max": 10, "Remaining": 10},
            "SingleEmail": {"Max": 15, "Remaining": 15}
        });

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/limits"))
            .respond_with(ResponseTemplate::new(200).set_body_json(high_usage_response))
            .mount(&mock_server)
            .await;

        let client = builder()
            .authenticate(auth)
            .build()
            .await
            .must_msg("Failed to build client");

        let limits = client
            .rest()
            .limits()
            .await
            .must_msg("Failed to get limits");
        assert!(limits.daily_api_requests.is_above_threshold(80.0));
        assert!((limits.daily_api_requests.percentage_used() - 90.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_limits_multiple_calls() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/limits"))
            .respond_with(ResponseTemplate::new(200).set_body_json(sample_limits_response()))
            .expect(3)
            .mount(&mock_server)
            .await;

        let client = builder()
            .authenticate(auth)
            .build()
            .await
            .must_msg("Failed to build client");

        // Make multiple calls to verify endpoint can be called repeatedly
        for _ in 0..3 {
            let limits = client
                .rest()
                .limits()
                .await
                .must_msg("Failed to get limits");
            assert_eq!(limits.daily_api_requests.max, 15000);
        }
    }

    #[tokio::test]
    async fn test_limits_cloned_handler() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/limits"))
            .respond_with(ResponseTemplate::new(200).set_body_json(sample_limits_response()))
            .expect(2)
            .mount(&mock_server)
            .await;

        let client = builder()
            .authenticate(auth)
            .build()
            .await
            .must_msg("Failed to build client");

        let handler1 = client.rest();
        let handler2 = handler1.clone();

        // Both handlers should work
        let limits1 = handler1.limits().await.must_msg("Failed with handler1");
        let limits2 = handler2.limits().await.must_msg("Failed with handler2");

        assert_eq!(
            limits1.daily_api_requests.max,
            limits2.daily_api_requests.max
        );
    }
}
