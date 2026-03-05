//! Query Plan API support.
//!
//! This module provides structures and methods for the Salesforce Query Plan API (`/query/?explain=`).
//! It allows developers to check the performance characteristics of their SOQL queries
//! (e.g., cardinality, cost, table scans) before executing them.
//!
//! Ideally used in CI/CD pipelines to prevent inefficient queries from reaching production.

use serde::Deserialize;

/// The top-level response from the Query Plan API.
#[derive(Debug, Clone, Deserialize)]
pub struct ExplainResponse {
    /// List of possible execution plans for the query.
    pub plans: Vec<QueryPlan>,
}

/// A single execution plan for a SOQL query.
///
/// Contains cost estimates and operation details (e.g., TableScan, IndexScan).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryPlan {
    /// Estimated number of records returned by this plan.
    pub cardinality: u64,

    /// Fields used in the plan (e.g., indexed fields).
    #[serde(default)]
    pub fields: Vec<String>,

    /// The primary operation type (e.g., "TableScan", "IndexScan").
    pub leading_operation_type: String,

    /// Additional notes or warnings about the plan (e.g., unindexed filter).
    #[serde(default)]
    pub notes: Vec<PlanNote>,

    /// Relative cost of this plan compared to others (lower is better).
    pub relative_cost: f64,

    /// Total number of records in the SObject table.
    pub sobject_cardinality: u64,

    /// The SObject type being queried.
    pub sobject_type: String,
}

/// A note or warning associated with a query plan.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanNote {
    /// Description of the note (e.g., "Not considered for indexing").
    pub description: String,

    /// Fields relevant to the note.
    #[serde(default)]
    pub fields: Vec<String>,

    /// The table or object ID related to the note.
    pub table_enum_or_id: String,
}

impl<A: crate::auth::Authenticator> crate::api::rest::RestHandler<A> {
    /// Retrieves the query execution plan for a SOQL query.
    ///
    /// The Query Plan API (`/query/?explain=`) allows developers to check the
    /// performance cost of a query before executing it. This is useful for
    /// identifying table scans and inefficient filters.
    ///
    /// # Arguments
    ///
    /// * `soql` - The SOQL query string to analyze.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Authentication fails
    /// - The HTTP request fails
    /// - The response cannot be deserialized
    #[cfg(feature = "nova")]
    pub async fn explain(&self, soql: &str) -> crate::error::Result<ExplainResponse> {
        self.execute_get(
            "/query",
            Some(&[("explain", soql)]),
            "Query Plan API request failed",
        )
        .await
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Must;

    #[test]
    fn test_deserialize_explain_response() {
        let json = serde_json::json!({
            "plans": [
                {
                    "cardinality": 50,
                    "fields": ["Name"],
                    "leadingOperationType": "TableScan",
                    "notes": [
                        {
                            "description": "Not indexed",
                            "fields": ["Name"],
                            "tableEnumOrId": "Account"
                        }
                    ],
                    "relativeCost": 1.5,
                    "sobjectCardinality": 1000,
                    "sobjectType": "Account"
                }
            ]
        });

        let response: ExplainResponse = serde_json::from_value(json).must();

        assert_eq!(response.plans.len(), 1);
        let plan = &response.plans[0];

        assert_eq!(plan.cardinality, 50);
        assert_eq!(plan.leading_operation_type, "TableScan");
        assert!((plan.relative_cost - 1.5).abs() < f64::EPSILON);
        assert_eq!(plan.sobject_cardinality, 1000);
        assert_eq!(plan.sobject_type, "Account");
        assert_eq!(plan.fields, vec!["Name"]);

        assert_eq!(plan.notes.len(), 1);
        let note = &plan.notes[0];
        assert_eq!(note.description, "Not indexed");
        assert_eq!(note.table_enum_or_id, "Account");
    }

    #[test]
    fn test_deserialize_explain_response_minimal() {
        // Test handling of optional/default fields
        let json = serde_json::json!({
            "plans": [
                {
                    "cardinality": 10,
                    "leadingOperationType": "IndexScan",
                    "relativeCost": 0.1,
                    "sobjectCardinality": 100,
                    "sobjectType": "Contact"
                }
            ]
        });

        let response: ExplainResponse = serde_json::from_value(json).must();
        let plan = &response.plans[0];

        assert!(plan.fields.is_empty());
        assert!(plan.notes.is_empty());
        assert_eq!(plan.leading_operation_type, "IndexScan");
    }

    #[tokio::test]
    async fn test_explain_api_call() {
        use crate::client::builder;
        use crate::test_support::{MockAuthenticator, Must};
        use wiremock::matchers::{method, path, query_param};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        let soql = "SELECT Id FROM Account";

        let json_response = serde_json::json!({
            "plans": [
                {
                    "cardinality": 50,
                    "fields": ["Name"],
                    "leadingOperationType": "TableScan",
                    "notes": [],
                    "relativeCost": 1.5,
                    "sobjectCardinality": 1000,
                    "sobjectType": "Account"
                }
            ]
        });

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/query"))
            .and(query_param("explain", soql))
            .respond_with(ResponseTemplate::new(200).set_body_json(json_response))
            .mount(&mock_server)
            .await;

        let response = client.rest().explain(soql).await.must();

        assert_eq!(response.plans.len(), 1);
        assert_eq!(response.plans[0].leading_operation_type, "TableScan");
    }
}
