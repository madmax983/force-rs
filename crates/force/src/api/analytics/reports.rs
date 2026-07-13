//! Report operations for the Analytics API.
//!
//! Synchronous and asynchronous report runs, instances, describe, listing,
//! ad-hoc queries, and report types.

use super::AnalyticsHandler;
use super::types::{
    ReportDescribe, ReportInstance, ReportListItem, ReportMetadataRequest, ReportResults,
    ReportTypeCategory,
};
use crate::error::Result;

impl<A: crate::auth::Authenticator> AnalyticsHandler<A> {
    /// Runs a report synchronously using its saved metadata.
    ///
    /// When `include_details` is `true`, record-level detail rows are populated
    /// in each fact-map entry's `rows`; a synchronous run returns at most 2000
    /// detail rows (use [`run_report_async`](Self::run_report_async) beyond
    /// that).
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response cannot be parsed.
    pub async fn run_report(
        &self,
        report_id: &str,
        include_details: bool,
    ) -> Result<ReportResults> {
        let details = if include_details { "true" } else { "false" };
        self.get(
            &format!("reports/{report_id}"),
            Some(&[("includeDetails", details)]),
            "Analytics run_report failed",
        )
        .await
    }

    /// Runs a report synchronously with dynamic metadata overrides.
    ///
    /// The provided [`ReportMetadataRequest`] overrides filters, groupings,
    /// scope, and similar settings at run time without saving the report.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response cannot be parsed.
    pub async fn run_report_with_metadata(
        &self,
        report_id: &str,
        include_details: bool,
        metadata: &ReportMetadataRequest,
    ) -> Result<ReportResults> {
        let details = if include_details { "true" } else { "false" };
        self.post(
            &format!("reports/{report_id}"),
            Some(&[("includeDetails", details)]),
            metadata,
            "Analytics run_report_with_metadata failed",
        )
        .await
    }

    /// Starts an asynchronous report run, returning the created instance summary.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response cannot be parsed.
    pub async fn run_report_async(
        &self,
        report_id: &str,
        include_details: bool,
    ) -> Result<ReportInstance> {
        let details = if include_details { "true" } else { "false" };
        self.post_empty(
            &format!("reports/{report_id}/instances"),
            Some(&[("includeDetails", details)]),
            "Analytics run_report_async failed",
        )
        .await
    }

    /// Starts an asynchronous report run with dynamic metadata overrides.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response cannot be parsed.
    pub async fn run_report_async_with_metadata(
        &self,
        report_id: &str,
        include_details: bool,
        metadata: &ReportMetadataRequest,
    ) -> Result<ReportInstance> {
        let details = if include_details { "true" } else { "false" };
        self.post(
            &format!("reports/{report_id}/instances"),
            Some(&[("includeDetails", details)]),
            metadata,
            "Analytics run_report_async_with_metadata failed",
        )
        .await
    }

    /// Lists the asynchronous instances of a report, most recent first.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response cannot be parsed.
    pub async fn list_report_instances(&self, report_id: &str) -> Result<Vec<ReportInstance>> {
        self.get(
            &format!("reports/{report_id}/instances"),
            None,
            "Analytics list_report_instances failed",
        )
        .await
    }

    /// Fetches the results of a specific asynchronous report instance.
    ///
    /// While the instance is pending the result carries a `status` of `New` or
    /// `Running` and no fact map; once complete it is a full
    /// [`ReportResults`].
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response cannot be parsed.
    pub async fn get_report_instance(
        &self,
        report_id: &str,
        instance_id: &str,
    ) -> Result<ReportResults> {
        self.get(
            &format!("reports/{report_id}/instances/{instance_id}"),
            None,
            "Analytics get_report_instance failed",
        )
        .await
    }

    /// Deletes a stored asynchronous report instance.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails.
    pub async fn delete_report_instance(&self, report_id: &str, instance_id: &str) -> Result<()> {
        self.delete_empty(
            &format!("reports/{report_id}/instances/{instance_id}"),
            "Analytics delete_report_instance failed",
        )
        .await
    }

    /// Describes a report's metadata, report-type metadata, and extended metadata.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response cannot be parsed.
    pub async fn describe_report(&self, report_id: &str) -> Result<ReportDescribe> {
        self.get(
            &format!("reports/{report_id}/describe"),
            None,
            "Analytics describe_report failed",
        )
        .await
    }

    /// Lists recently viewed reports.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response cannot be parsed.
    pub async fn list_reports(&self) -> Result<Vec<ReportListItem>> {
        self.get("reports", None, "Analytics list_reports failed")
            .await
    }

    /// Runs an ad-hoc report without a saved report definition.
    ///
    /// The metadata must describe at least a report type, format, columns or
    /// aggregates, and groupings.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response cannot be parsed.
    pub async fn query_report(&self, metadata: &ReportMetadataRequest) -> Result<ReportResults> {
        self.post(
            "reports/query",
            None,
            metadata,
            "Analytics query_report failed",
        )
        .await
    }

    /// Lists available report types, grouped by category.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response cannot be parsed.
    pub async fn list_report_types(&self) -> Result<Vec<ReportTypeCategory>> {
        self.get("reportTypes", None, "Analytics list_report_types failed")
            .await
    }

    /// Describes a single report type.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response cannot be parsed.
    pub async fn describe_report_type(&self, report_type: &str) -> Result<ReportDescribe> {
        self.get(
            &format!("reportTypes/{report_type}"),
            None,
            "Analytics describe_report_type failed",
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use crate::api::analytics::types::{
        FilterOperator, InstanceStatus, ReportFilter, ReportFormat, ReportMetadata,
        ReportMetadataRequest,
    };
    use crate::client::{ForceClient, builder};
    use crate::test_utils::mock_auth::MockAuthenticator;
    use crate::test_utils::must::Must;
    use wiremock::matchers::{body_partial_json, header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn setup() -> (MockServer, ForceClient<MockAuthenticator>) {
        let server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &server.uri());
        let client = builder().authenticate(auth).build().await.must();
        (server, client)
    }

    fn summary_report_body() -> serde_json::Value {
        serde_json::json!({
            "attributes": {
                "reportId": "00O3000000B5Yn2",
                "reportName": "Opportunities by Stage",
                "type": "Report"
            },
            "allData": true,
            "hasDetailRows": true,
            "reportMetadata": {
                "id": "00O3000000B5Yn2",
                "name": "Opportunities by Stage",
                "reportFormat": "SUMMARY",
                "reportFilters": [
                    { "column": "StageName", "operator": "notEqual", "value": "Closed Lost" }
                ]
            },
            "groupingsDown": { "groupings": [
                { "key": "0", "label": "Prospecting", "value": "Prospecting", "groupings": [] }
            ] },
            "groupingsAcross": { "groupings": [] },
            "factMap": {
                "0!T": {
                    "aggregates": [ { "label": "$1,500,000", "value": 1_500_000 } ],
                    "rows": [ { "dataCells": [
                        { "label": "Acme", "value": "Acme" },
                        { "label": "$500,000", "value": 500_000 }
                    ] } ]
                },
                "T!T": {
                    "aggregates": [ { "label": "$1,500,000", "value": 1_500_000 } ],
                    "rows": []
                }
            }
        })
    }

    #[tokio::test]
    async fn test_run_report_success() {
        let (server, client) = setup().await;

        Mock::given(method("GET"))
            .and(path(
                "/services/data/v67.0/analytics/reports/00O3000000B5Yn2",
            ))
            .and(header("Authorization", "Bearer test_token"))
            .and(query_param("includeDetails", "true"))
            .respond_with(ResponseTemplate::new(200).set_body_json(summary_report_body()))
            .expect(1)
            .mount(&server)
            .await;

        let results = client
            .analytics()
            .run_report("00O3000000B5Yn2", true)
            .await
            .must();

        assert!(results.all_data);
        assert!(results.has_detail_rows);
        assert!(results.fact_map.contains_key("T!T"));
        assert_eq!(
            results.fact_map["0!T"].aggregates[0].value,
            serde_json::json!(1_500_000)
        );
        let meta = results.report_metadata.as_ref().must();
        assert_eq!(meta.report_format, Some(ReportFormat::Summary));
        assert_eq!(meta.report_filters[0].operator, FilterOperator::NotEqual);
    }

    #[tokio::test]
    async fn test_run_report_without_details() {
        let (server, client) = setup().await;

        Mock::given(method("GET"))
            .and(path("/services/data/v67.0/analytics/reports/00O000"))
            .and(query_param("includeDetails", "false"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "allData": true,
                "hasDetailRows": false,
                "factMap": { "T!T": { "aggregates": [ { "label": "3", "value": 3 } ], "rows": [] } }
            })))
            .mount(&server)
            .await;

        let results = client.analytics().run_report("00O000", false).await.must();
        assert!(!results.has_detail_rows);
        assert!(results.fact_map["T!T"].rows.is_empty());
    }

    #[tokio::test]
    async fn test_run_report_with_metadata_posts_body() {
        let (server, client) = setup().await;

        Mock::given(method("POST"))
            .and(path(
                "/services/data/v67.0/analytics/reports/00O3000000B5Yn2",
            ))
            .and(query_param("includeDetails", "true"))
            .and(body_partial_json(serde_json::json!({
                "reportMetadata": {
                    "reportFilters": [
                        { "column": "StageName", "operator": "equals", "value": "Closed Won" }
                    ]
                }
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(summary_report_body()))
            .expect(1)
            .mount(&server)
            .await;

        let req = ReportMetadataRequest::new(ReportMetadata {
            report_filters: vec![ReportFilter::new(
                "StageName",
                FilterOperator::Equals,
                "Closed Won",
            )],
            ..Default::default()
        });

        let results = client
            .analytics()
            .run_report_with_metadata("00O3000000B5Yn2", true, &req)
            .await
            .must();
        assert!(results.all_data);
    }

    #[tokio::test]
    async fn test_run_report_async_creates_instance() {
        let (server, client) = setup().await;

        Mock::given(method("POST"))
            .and(path("/services/data/v67.0/analytics/reports/00O/instances"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "0LG000000000001",
                "status": "New",
                "requestDate": "2026-07-12T18:02:11Z",
                "queryable": false,
                "hasDetailRows": false,
                "url": "/services/data/v67.0/analytics/reports/00O/instances/0LG000000000001"
            })))
            .mount(&server)
            .await;

        let instance = client
            .analytics()
            .run_report_async("00O", false)
            .await
            .must();
        assert_eq!(instance.id, "0LG000000000001");
        assert_eq!(instance.status, InstanceStatus::New);
    }

    #[tokio::test]
    async fn test_list_report_instances() {
        let (server, client) = setup().await;

        Mock::given(method("GET"))
            .and(path("/services/data/v67.0/analytics/reports/00O/instances"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                { "id": "0LG2", "status": "Success" },
                { "id": "0LG1", "status": "Running" }
            ])))
            .mount(&server)
            .await;

        let instances = client.analytics().list_report_instances("00O").await.must();
        assert_eq!(instances.len(), 2);
        assert_eq!(instances[0].status, InstanceStatus::Success);
        assert_eq!(instances[1].status, InstanceStatus::Running);
    }

    #[tokio::test]
    async fn test_get_report_instance_status_transition() {
        let (server, client) = setup().await;

        // Pending instance: status present, no fact map.
        Mock::given(method("GET"))
            .and(path(
                "/services/data/v67.0/analytics/reports/00O/instances/0LGpending",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "attributes": { "id": "0LGpending", "status": "Running" },
                "status": "Running"
            })))
            .mount(&server)
            .await;

        let pending = client
            .analytics()
            .get_report_instance("00O", "0LGpending")
            .await
            .must();
        assert_eq!(pending.status, Some(InstanceStatus::Running));
        assert!(pending.fact_map.is_empty());

        // Completed instance: full results.
        Mock::given(method("GET"))
            .and(path(
                "/services/data/v67.0/analytics/reports/00O/instances/0LGdone",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(summary_report_body()))
            .mount(&server)
            .await;

        let done = client
            .analytics()
            .get_report_instance("00O", "0LGdone")
            .await
            .must();
        assert!(done.fact_map.contains_key("T!T"));
    }

    #[tokio::test]
    async fn test_delete_report_instance() {
        let (server, client) = setup().await;

        Mock::given(method("DELETE"))
            .and(path(
                "/services/data/v67.0/analytics/reports/00O/instances/0LG",
            ))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;

        client
            .analytics()
            .delete_report_instance("00O", "0LG")
            .await
            .must();
    }

    #[tokio::test]
    async fn test_describe_report() {
        let (server, client) = setup().await;

        Mock::given(method("GET"))
            .and(path("/services/data/v67.0/analytics/reports/00O/describe"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "reportMetadata": { "id": "00O", "reportFormat": "MATRIX" },
                "reportTypeMetadata": { "categories": [] },
                "reportExtendedMetadata": {
                    "detailColumnInfo": {},
                    "aggregateColumnInfo": {},
                    "groupingColumnInfo": {}
                }
            })))
            .mount(&server)
            .await;

        let describe = client.analytics().describe_report("00O").await.must();
        assert_eq!(
            describe.report_metadata.report_format,
            Some(ReportFormat::Matrix)
        );
        assert!(describe.report_type_metadata.is_object());
        assert!(describe.report_extended_metadata.is_some());
    }

    #[tokio::test]
    async fn test_list_reports() {
        let (server, client) = setup().await;

        Mock::given(method("GET"))
            .and(path("/services/data/v67.0/analytics/reports"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {
                    "id": "00O1",
                    "name": "Closed Won This Quarter",
                    "url": "/services/data/v67.0/analytics/reports/00O1",
                    "describeUrl": "/services/data/v67.0/analytics/reports/00O1/describe",
                    "instancesUrl": "/services/data/v67.0/analytics/reports/00O1/instances"
                }
            ])))
            .mount(&server)
            .await;

        let reports = client.analytics().list_reports().await.must();
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].id.as_deref(), Some("00O1"));
        assert_eq!(reports[0].name.as_deref(), Some("Closed Won This Quarter"));
        assert!(reports[0].describe_url.is_some());
    }

    #[tokio::test]
    async fn test_query_report() {
        let (server, client) = setup().await;

        Mock::given(method("POST"))
            .and(path("/services/data/v67.0/analytics/reports/query"))
            .and(body_partial_json(serde_json::json!({
                "reportMetadata": { "reportFormat": "TABULAR" }
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "allData": true,
                "hasDetailRows": true,
                "factMap": { "T!T": { "aggregates": [], "rows": [] } }
            })))
            .mount(&server)
            .await;

        let req = ReportMetadataRequest::new(ReportMetadata {
            report_format: Some(ReportFormat::Tabular),
            detail_columns: vec!["NAME".to_string()],
            ..Default::default()
        });
        let results = client.analytics().query_report(&req).await.must();
        assert!(results.all_data);
    }

    #[tokio::test]
    async fn test_list_report_types() {
        let (server, client) = setup().await;

        Mock::given(method("GET"))
            .and(path("/services/data/v67.0/analytics/reportTypes"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {
                    "label": "Accounts & Contacts",
                    "reportTypes": [
                        { "type": "AccountList", "label": "Accounts", "isAvailable": true }
                    ]
                }
            ])))
            .mount(&server)
            .await;

        let types = client.analytics().list_report_types().await.must();
        assert_eq!(types.len(), 1);
        assert_eq!(types[0].label.as_deref(), Some("Accounts & Contacts"));
        assert_eq!(
            types[0].report_types[0].type_name.as_deref(),
            Some("AccountList")
        );
    }

    #[tokio::test]
    async fn test_describe_report_type() {
        let (server, client) = setup().await;

        Mock::given(method("GET"))
            .and(path(
                "/services/data/v67.0/analytics/reportTypes/AccountList",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "reportMetadata": { "reportFormat": "TABULAR" },
                "reportTypeMetadata": { "categories": [] },
                "reportExtendedMetadata": {}
            })))
            .mount(&server)
            .await;

        let describe = client
            .analytics()
            .describe_report_type("AccountList")
            .await
            .must();
        assert_eq!(
            describe.report_metadata.report_format,
            Some(ReportFormat::Tabular)
        );
    }

    #[tokio::test]
    async fn test_run_report_server_error() {
        let (server, client) = setup().await;

        Mock::given(method("GET"))
            .and(path("/services/data/v67.0/analytics/reports/00Obad"))
            .respond_with(
                ResponseTemplate::new(404).set_body_json(serde_json::json!([{
                    "message": "The requested resource does not exist",
                    "errorCode": "NOT_FOUND"
                }])),
            )
            .mount(&server)
            .await;

        let result = client.analytics().run_report("00Obad", true).await;
        let Err(err) = result else {
            panic!("Expected an error");
        };
        assert!(matches!(
            err,
            crate::error::ForceError::Api(_) | crate::error::ForceError::Http(_)
        ));
    }
}
