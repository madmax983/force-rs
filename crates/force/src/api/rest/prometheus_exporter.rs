//! Prometheus Metrics Exporter for Organization Limits.
//!
//! This module provides a utility to format Salesforce `OrgLimits` into the
//! standard Prometheus exposition format, making it easy to expose org limits
//! on a `/metrics` endpoint for monitoring tools like Grafana.

use crate::api::rest::limits::{LimitInfo, OrgLimits};
use std::fmt::Write;

/// Configuration options for the Prometheus exporter.
#[derive(Debug, Clone)]
pub struct PrometheusExportOptions {
    /// A prefix for all metric names. Defaults to `salesforce_org_limit`.
    pub metric_prefix: String,
    /// A set of custom labels to apply to all metrics, formatted as `key="value"`.
    /// E.g., `vec!["org=\"prod\"", "region=\"us-east-1\""]`.
    pub custom_labels: Vec<String>,
}

impl Default for PrometheusExportOptions {
    fn default() -> Self {
        Self {
            metric_prefix: "salesforce_org_limit".to_string(),
            custom_labels: Vec::new(),
        }
    }
}

/// Helper to write a single LimitInfo to the Prometheus format string.
fn write_limit_info(
    output: &mut String,
    prefix: &str,
    limit_name: &str,
    limit_info: &LimitInfo,
    labels_str: &str,
) {
    // Determine if we need a comma before adding the limit_name label
    let comma = if labels_str.is_empty() { "" } else { "," };
    let labels = format!("{}{comma}limit=\"{limit_name}\"", labels_str);

    // Max metric
    let _ = writeln!(
        output,
        "{prefix}_max{{{labels}}} {max}",
        max = limit_info.max
    );

    // Remaining metric
    let _ = writeln!(
        output,
        "{prefix}_remaining{{{labels}}} {remaining}",
        remaining = limit_info.remaining
    );

    // Used metric (either explicit or calculated)
    let used = limit_info
        .used
        .unwrap_or_else(|| limit_info.max - limit_info.remaining);

    let _ = writeln!(output, "{prefix}_used{{{labels}}} {used}",);
}

/// Converts a camelCase or PascalCase string to snake_case.
fn to_snake_case(s: &str) -> String {
    let mut snake = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                snake.push('_');
            }
            snake.push(c.to_ascii_lowercase());
        } else {
            snake.push(c);
        }
    }
    snake
}

/// Exports `OrgLimits` into a Prometheus text-based exposition format string.
///
/// # Arguments
///
/// * `limits` - The `OrgLimits` instance to export.
/// * `options` - Customization options for metric prefix and labels.
///
/// # Returns
///
/// A `String` containing the Prometheus metrics payload.
#[allow(clippy::too_many_lines)]
pub fn export_limits_to_prometheus(
    limits: &OrgLimits,
    options: &PrometheusExportOptions,
) -> String {
    let mut output = String::with_capacity(4096);
    let prefix = &options.metric_prefix;

    let labels_str = if options.custom_labels.is_empty() {
        String::new()
    } else {
        options.custom_labels.join(",")
    };

    // Add TYPE and HELP for the max metric
    let _ = writeln!(
        output,
        "# HELP {prefix}_max Maximum allowed for the specified Salesforce limit"
    );
    let _ = writeln!(output, "# TYPE {prefix}_max gauge");

    // Add TYPE and HELP for the remaining metric
    let _ = writeln!(
        output,
        "# HELP {prefix}_remaining Remaining capacity before hitting the Salesforce limit"
    );
    let _ = writeln!(output, "# TYPE {prefix}_remaining gauge");

    // Add TYPE and HELP for the used metric
    let _ = writeln!(
        output,
        "# HELP {prefix}_used Amount currently used for the Salesforce limit"
    );
    let _ = writeln!(output, "# TYPE {prefix}_used gauge");

    // Known limits
    write_limit_info(
        &mut output,
        prefix,
        "daily_api_requests",
        &limits.daily_api_requests,
        &labels_str,
    );
    write_limit_info(
        &mut output,
        prefix,
        "daily_async_apex_executions",
        &limits.daily_async_apex_executions,
        &labels_str,
    );
    write_limit_info(
        &mut output,
        prefix,
        "daily_batch_apex_executions",
        &limits.daily_batch_apex_executions,
        &labels_str,
    );
    write_limit_info(
        &mut output,
        prefix,
        "daily_durable_generic_streaming_api_events",
        &limits.daily_durable_generic_streaming_api_events,
        &labels_str,
    );
    write_limit_info(
        &mut output,
        prefix,
        "daily_durable_streaming_api_events",
        &limits.daily_durable_streaming_api_events,
        &labels_str,
    );
    write_limit_info(
        &mut output,
        prefix,
        "daily_generic_streaming_api_events",
        &limits.daily_generic_streaming_api_events,
        &labels_str,
    );
    write_limit_info(
        &mut output,
        prefix,
        "daily_streaming_api_events",
        &limits.daily_streaming_api_events,
        &labels_str,
    );
    write_limit_info(
        &mut output,
        prefix,
        "daily_workflow_emails",
        &limits.daily_workflow_emails,
        &labels_str,
    );
    write_limit_info(
        &mut output,
        prefix,
        "data_storage_mb",
        &limits.data_storage_mb,
        &labels_str,
    );
    write_limit_info(
        &mut output,
        prefix,
        "file_storage_mb",
        &limits.file_storage_mb,
        &labels_str,
    );
    write_limit_info(
        &mut output,
        prefix,
        "hourly_async_report_runs",
        &limits.hourly_async_report_runs,
        &labels_str,
    );
    write_limit_info(
        &mut output,
        prefix,
        "hourly_dashboard_refreshes",
        &limits.hourly_dashboard_refreshes,
        &labels_str,
    );
    write_limit_info(
        &mut output,
        prefix,
        "hourly_dashboard_results",
        &limits.hourly_dashboard_results,
        &labels_str,
    );
    write_limit_info(
        &mut output,
        prefix,
        "hourly_dashboard_statuses",
        &limits.hourly_dashboard_statuses,
        &labels_str,
    );
    write_limit_info(
        &mut output,
        prefix,
        "hourly_long_term_id_mapping",
        &limits.hourly_long_term_id_mapping,
        &labels_str,
    );
    write_limit_info(
        &mut output,
        prefix,
        "hourly_managed_content_public_requests",
        &limits.hourly_managed_content_public_requests,
        &labels_str,
    );
    write_limit_info(
        &mut output,
        prefix,
        "hourly_o_data_callout",
        &limits.hourly_o_data_callout,
        &labels_str,
    );
    write_limit_info(
        &mut output,
        prefix,
        "hourly_short_term_id_mapping",
        &limits.hourly_short_term_id_mapping,
        &labels_str,
    );
    write_limit_info(
        &mut output,
        prefix,
        "hourly_time_based_workflow",
        &limits.hourly_time_based_workflow,
        &labels_str,
    );
    write_limit_info(
        &mut output,
        prefix,
        "mass_email",
        &limits.mass_email,
        &labels_str,
    );
    write_limit_info(
        &mut output,
        prefix,
        "single_email",
        &limits.single_email,
        &labels_str,
    );

    // Dynamic additional limits
    for (name, info) in &limits.additional_limits {
        let snake_name = to_snake_case(name);
        write_limit_info(&mut output, prefix, &snake_name, info, &labels_str);
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::rest::limits::LimitInfo;
    use std::collections::HashMap;

    fn create_mock_limits() -> OrgLimits {
        let default_limit = LimitInfo {
            max: 1000,
            remaining: 500,
            used: None,
        };

        let mut additional = HashMap::new();
        additional.insert(
            "FutureNewLimit".to_string(),
            LimitInfo {
                max: 5000,
                remaining: 4500,
                used: Some(500),
            },
        );

        OrgLimits {
            daily_api_requests: LimitInfo {
                max: 15000,
                remaining: 14000,
                used: Some(1000),
            },
            daily_async_apex_executions: default_limit.clone(),
            daily_batch_apex_executions: default_limit.clone(),
            daily_durable_generic_streaming_api_events: default_limit.clone(),
            daily_durable_streaming_api_events: default_limit.clone(),
            daily_generic_streaming_api_events: default_limit.clone(),
            daily_streaming_api_events: default_limit.clone(),
            daily_workflow_emails: default_limit.clone(),
            data_storage_mb: default_limit.clone(),
            file_storage_mb: default_limit.clone(),
            hourly_async_report_runs: default_limit.clone(),
            hourly_dashboard_refreshes: default_limit.clone(),
            hourly_dashboard_results: default_limit.clone(),
            hourly_dashboard_statuses: default_limit.clone(),
            hourly_long_term_id_mapping: default_limit.clone(),
            hourly_managed_content_public_requests: default_limit.clone(),
            hourly_o_data_callout: default_limit.clone(),
            hourly_short_term_id_mapping: default_limit.clone(),
            hourly_time_based_workflow: default_limit.clone(),
            mass_email: default_limit.clone(),
            single_email: default_limit,
            additional_limits: additional,
        }
    }

    #[test]
    fn test_export_default_options() {
        let limits = create_mock_limits();
        let options = PrometheusExportOptions::default();
        let metrics = export_limits_to_prometheus(&limits, &options);

        // Check for HELP and TYPE headers
        assert!(metrics.contains("# HELP salesforce_org_limit_max"));
        assert!(metrics.contains("# TYPE salesforce_org_limit_max gauge"));
        assert!(metrics.contains("# HELP salesforce_org_limit_used"));

        // Check explicit known limit
        assert!(metrics.contains("salesforce_org_limit_max{limit=\"daily_api_requests\"} 15000\n"));
        assert!(
            metrics
                .contains("salesforce_org_limit_remaining{limit=\"daily_api_requests\"} 14000\n")
        );
        assert!(metrics.contains("salesforce_org_limit_used{limit=\"daily_api_requests\"} 1000\n"));

        // Check fallback used calculation for other limits
        assert!(metrics.contains("salesforce_org_limit_used{limit=\"mass_email\"} 500\n"));

        // Check dynamic additional limit with snake casing
        assert!(metrics.contains("salesforce_org_limit_max{limit=\"future_new_limit\"} 5000\n"));
    }

    #[test]
    fn test_export_custom_labels_and_prefix() {
        let limits = create_mock_limits();
        let options = PrometheusExportOptions {
            metric_prefix: "sf_limit".to_string(),
            custom_labels: vec!["env=\"prod\"".to_string(), "region=\"us\"".to_string()],
        };
        let metrics = export_limits_to_prometheus(&limits, &options);

        assert!(metrics.contains("# TYPE sf_limit_max gauge"));
        assert!(metrics.contains(
            "sf_limit_max{env=\"prod\",region=\"us\",limit=\"daily_api_requests\"} 15000\n"
        ));
    }

    #[test]
    fn test_to_snake_case() {
        assert_eq!(to_snake_case("FutureNewLimit"), "future_new_limit");
        assert_eq!(to_snake_case("anotherLimit123"), "another_limit123");
        assert_eq!(to_snake_case("APIRequests"), "a_p_i_requests"); // Simple logic is fine for dynamic limits
    }
}
