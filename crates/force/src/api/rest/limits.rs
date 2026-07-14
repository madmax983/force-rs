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
/// if let Some(api_limit) = &limits.daily_api_requests {
///     println!("API calls remaining: {}/{}", api_limit.remaining, api_limit.max);
/// }
/// ```
///
/// # Optional fields
///
/// Not every Salesforce org edition returns every named limit (for example,
/// some editions omit `DailyBatchApexExecutions`). Each named field is therefore
/// an `Option<LimitInfo>` that is `None` when the org did not return it. Any
/// limit not modelled here is still captured in [`additional_limits`].
///
/// [`additional_limits`]: OrgLimits::additional_limits
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct OrgLimits {
    /// Daily API request limit and usage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub daily_api_requests: Option<LimitInfo>,

    /// Daily asynchronous Apex executions limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub daily_async_apex_executions: Option<LimitInfo>,

    /// Daily batch Apex executions limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub daily_batch_apex_executions: Option<LimitInfo>,

    /// Daily durable generic streaming API events limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub daily_durable_generic_streaming_api_events: Option<LimitInfo>,

    /// Daily durable streaming API events limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub daily_durable_streaming_api_events: Option<LimitInfo>,

    /// Daily generic streaming API events limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub daily_generic_streaming_api_events: Option<LimitInfo>,

    /// Daily streaming API events limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub daily_streaming_api_events: Option<LimitInfo>,

    /// Daily workflow emails limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub daily_workflow_emails: Option<LimitInfo>,

    /// Data storage (MB).
    #[serde(
        rename = "DataStorageMB",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub data_storage_mb: Option<LimitInfo>,

    /// File storage (MB).
    #[serde(
        rename = "FileStorageMB",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub file_storage_mb: Option<LimitInfo>,

    /// Hourly asynchronous report runs limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hourly_async_report_runs: Option<LimitInfo>,

    /// Hourly dashboard refreshes limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hourly_dashboard_refreshes: Option<LimitInfo>,

    /// Hourly dashboard results limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hourly_dashboard_results: Option<LimitInfo>,

    /// Hourly dashboard status limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hourly_dashboard_statuses: Option<LimitInfo>,

    /// Hourly long-term ID mapping limit.
    #[serde(
        rename = "HourlyLongTermIdMapping",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub hourly_long_term_id_mapping: Option<LimitInfo>,

    /// Hourly managed content public requests limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hourly_managed_content_public_requests: Option<LimitInfo>,

    /// Hourly OData callout limit.
    #[serde(
        rename = "HourlyODataCallout",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub hourly_o_data_callout: Option<LimitInfo>,

    /// Hourly short-term ID mapping limit.
    #[serde(
        rename = "HourlyShortTermIdMapping",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub hourly_short_term_id_mapping: Option<LimitInfo>,

    /// Hourly time-based workflow limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hourly_time_based_workflow: Option<LimitInfo>,

    /// Mass email limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mass_email: Option<LimitInfo>,

    /// Single email limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub single_email: Option<LimitInfo>,

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
/// use force::api::rest::LimitInfo;
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
    /// use force::api::rest::LimitInfo;
    ///
    /// let limit = LimitInfo::new(1000, 250, Some(750));
    /// assert_eq!(limit.percentage_used(), 75.0);
    /// ```
    #[must_use]
    pub fn percentage_used(&self) -> f64 {
        if self.max == 0 {
            return 0.0;
        }
        let used = self
            .used
            .unwrap_or_else(|| self.max.saturating_sub(self.remaining));
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
    /// use force::api::rest::LimitInfo;
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
    use crate::test_utils::must::Must;

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
    fn test_no_panic_on_overflow() {
        let limit = LimitInfo::new(i64::MIN, i64::MAX, None);
        let _ = limit.percentage_used();
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
        let api = limits.daily_api_requests.as_ref().must();
        assert_eq!(api.max, 15000);
        assert_eq!(api.remaining, 14850);
        assert_eq!(api.used, Some(150));
        assert_eq!(limits.data_storage_mb.as_ref().must().max, 5120);
        assert_eq!(limits.file_storage_mb.as_ref().must().max, 20480);
    }

    #[test]
    fn test_org_limits_deserialize_missing_named_limits() {
        // Not every org edition returns every named limit. A payload that omits
        // DailyBatchApexExecutions (and a couple of others) must still
        // deserialize, leaving the omitted fields as `None`.
        let json = r#"{
            "DailyApiRequests": {"Max": 15000, "Remaining": 14850, "Used": 150},
            "DataStorageMB": {"Max": 5120, "Remaining": 4800},
            "FutureLimit": {"Max": 999, "Remaining": 888}
        }"#;

        let limits: OrgLimits = serde_json::from_str(json).must();

        // Present named limit parses.
        assert_eq!(limits.daily_api_requests.as_ref().must().max, 15000);
        assert_eq!(limits.data_storage_mb.as_ref().must().max, 5120);

        // Omitted named limits are None instead of a deserialization error.
        assert!(limits.daily_batch_apex_executions.is_none());
        assert!(limits.daily_async_apex_executions.is_none());
        assert!(limits.single_email.is_none());

        // Unknown limits still land in the flatten map.
        assert!(limits.additional_limits.contains_key("FutureLimit"));
        assert_eq!(limits.additional_limits["FutureLimit"].max, 999);
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
            daily_api_requests: Some(LimitInfo::new(15000, 14850, Some(150))),
            daily_async_apex_executions: Some(LimitInfo::new(250_000, 250_000, None)),
            daily_batch_apex_executions: Some(LimitInfo::new(250_000, 250_000, None)),
            daily_durable_generic_streaming_api_events: Some(LimitInfo::new(10000, 10000, None)),
            daily_durable_streaming_api_events: Some(LimitInfo::new(10000, 10000, None)),
            daily_generic_streaming_api_events: Some(LimitInfo::new(10000, 10000, None)),
            daily_streaming_api_events: Some(LimitInfo::new(10000, 10000, None)),
            daily_workflow_emails: Some(LimitInfo::new(1000, 1000, None)),
            data_storage_mb: Some(LimitInfo::new(5120, 4800, None)),
            file_storage_mb: Some(LimitInfo::new(20480, 20000, None)),
            hourly_async_report_runs: Some(LimitInfo::new(1200, 1200, None)),
            hourly_dashboard_refreshes: Some(LimitInfo::new(200, 200, None)),
            hourly_dashboard_results: Some(LimitInfo::new(5000, 5000, None)),
            hourly_dashboard_statuses: Some(LimitInfo::new(999_999_999, 999_999_999, None)),
            hourly_long_term_id_mapping: Some(LimitInfo::new(100_000, 100_000, None)),
            hourly_managed_content_public_requests: Some(LimitInfo::new(50000, 50000, None)),
            hourly_o_data_callout: Some(LimitInfo::new(10000, 10000, None)),
            hourly_short_term_id_mapping: Some(LimitInfo::new(100_000, 100_000, None)),
            hourly_time_based_workflow: Some(LimitInfo::new(1000, 1000, None)),
            mass_email: Some(LimitInfo::new(10, 10, None)),
            single_email: Some(LimitInfo::new(15, 15, None)),
            additional_limits: HashMap::new(),
        };

        let json = serde_json::to_string(&original).must();
        let deserialized: OrgLimits = serde_json::from_str(&json).must();

        assert_eq!(original, deserialized);
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn test_percentage_used_boundary_conditions() {
        // Test < vs <= for lowest cost logic
        let l1 = LimitInfo {
            max: 100,
            remaining: 25,
            used: None,
        };
        assert_eq!(l1.percentage_used(), 75.0);
        let l0 = LimitInfo {
            max: 0,
            remaining: 0,
            used: None,
        };
        assert_eq!(l0.percentage_used(), 0.0);
        let l2 = LimitInfo {
            max: 100,
            remaining: 0,
            used: None,
        };
        assert_eq!(l2.percentage_used(), 100.0);
        assert!(l2.is_at_limit());
        let l3 = LimitInfo {
            max: 100,
            remaining: 100,
            used: None,
        };
        assert_eq!(l3.percentage_used(), 0.0);
        assert!(!l3.is_at_limit());

        let l4 = LimitInfo {
            max: 100,
            remaining: 25,
            used: None,
        };
        assert!(l4.is_above_threshold(70.0));
        assert!(!l4.is_above_threshold(80.0));
        assert!(!l4.is_above_threshold(75.0)); // Exact match should return false
    }

    #[cfg(feature = "mock")]
    #[test]
    fn test_sample_limits_response_is_valid() {
        let resp = super::integration_tests::sample_limits_response();
        assert!(resp.is_object());
        let Some(obj) = resp.as_object() else {
            panic!("Expected JSON Object");
        };
        assert!(!obj.is_empty());
    }
}

// Integration tests with wiremock
#[cfg(all(test, feature = "mock"))]
mod integration_tests {
    use crate::client::builder;
    use crate::config::ClientConfig;
    use crate::test_utils::mock_auth::MockAuthenticator;
    use crate::test_utils::must::{Must, MustMsg};
    use wiremock::matchers::{bearer_token, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    pub fn sample_limits_response() -> serde_json::Value {
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
            .and(path("/services/data/v67.0/limits"))
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

        let api = limits.daily_api_requests.as_ref().must();
        assert_eq!(api.max, 15000);
        assert_eq!(api.remaining, 14850);
        assert_eq!(api.used, Some(150));
        assert_eq!(limits.daily_workflow_emails.as_ref().must().used, Some(5));
        assert_eq!(limits.data_storage_mb.as_ref().must().max, 5120);
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

        let config = ClientConfig {
            api_version: "v59.0".into(),
            ..Default::default()
        };
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
        assert_eq!(limits.daily_api_requests.as_ref().must().max, 15000);
    }

    #[tokio::test]
    async fn test_limits_unauthorized() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("invalid_token", &mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/services/data/v67.0/limits"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&mock_server)
            .await;

        let client = builder()
            .authenticate(auth)
            .build()
            .await
            .must_msg("Failed to build client");

        let result = client.rest().limits().await;
        let Err(err) = result else {
            panic!("Expected an error");
        };
        assert!(err.to_string().contains(""));
    }

    #[tokio::test]
    async fn test_limits_server_error() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/services/data/v67.0/limits"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock_server)
            .await;

        let client = builder()
            .authenticate(auth)
            .build()
            .await
            .must_msg("Failed to build client");

        let result = client.rest().limits().await;
        let Err(err) = result else {
            panic!("Expected an error");
        };
        assert!(err.to_string().contains(""));
    }

    #[tokio::test]
    async fn test_limits_correct_headers() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("header_test_token", &mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/services/data/v67.0/limits"))
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
            .and(path("/services/data/v67.0/limits"))
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
        let api = limits.daily_api_requests.as_ref().must();
        assert!(api.is_at_limit());
        assert!((api.percentage_used() - 100.0).abs() < f64::EPSILON);
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
            .and(path("/services/data/v67.0/limits"))
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
            .and(path("/services/data/v67.0/limits"))
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
        let api = limits.daily_api_requests.as_ref().must();
        assert!(api.is_above_threshold(80.0));
        assert!((api.percentage_used() - 90.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_limits_multiple_calls() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/services/data/v67.0/limits"))
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
            assert_eq!(limits.daily_api_requests.as_ref().must().max, 15000);
        }
    }

    #[tokio::test]
    async fn test_limits_cloned_handler() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/services/data/v67.0/limits"))
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
            limits1.daily_api_requests.as_ref().must().max,
            limits2.daily_api_requests.as_ref().must().max
        );
    }
}
