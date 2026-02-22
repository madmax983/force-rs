//! Query Plan Explain API.
//!
//! This module provides access to the Salesforce Query Plan tool, which helps
//! developers understand and optimize SOQL query performance.
//!
//! # Availability
//!
//! This feature is available behind the `nova` feature flag.

use crate::error::Result;
use serde::Deserialize;

/// Response from the Query Explain API.
#[derive(Debug, Clone, Deserialize)]
pub struct QueryPlanResponse {
    /// List of possible execution plans.
    pub plans: Vec<QueryPlan>,
}

/// A single execution plan for a SOQL query.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryPlan {
    /// Estimated cardinality (number of rows) for this plan.
    pub cardinality: u64,
    /// Fields used in the index or filter.
    pub fields: Option<Vec<String>>,
    /// The leading operation type (e.g., TableScan, Index).
    pub leading_operation_type: String,
    /// Notes regarding the plan optimization.
    pub notes: Option<Vec<QueryPlanNote>>,
    /// Relative cost of this plan compared to others.
    pub relative_cost: f64,
    /// Total number of records in the SObject table.
    pub sobject_cardinality: u64,
    /// The SObject type being queried.
    pub sobject_type: String,
}

/// A note attached to a query plan.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryPlanNote {
    /// Description of the note.
    pub description: Option<String>,
    /// Fields involved in the note.
    pub fields: Option<Vec<String>>,
    /// Table enum or ID.
    pub table_enum_or_id: String,
}

impl<A: crate::auth::Authenticator> super::RestHandler<A> {
    /// Explains the execution plan for a SOQL query.
    ///
    /// This method calls the Salesforce Query Plan API to retrieve details about
    /// how the query will be executed (e.g., TableScan vs Index).
    ///
    /// # Arguments
    ///
    /// * `soql` - The SOQL query string to explain.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Authentication fails
    /// - The HTTP request fails
    /// - The response cannot be deserialized
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let plan = client.rest().explain("SELECT Id FROM Account WHERE Name = 'Acme'").await?;
    /// for p in plan.plans {
    ///     println!("Type: {}, Cost: {}", p.leading_operation_type, p.relative_cost);
    /// }
    /// ```
    pub async fn explain(&self, soql: &str) -> Result<QueryPlanResponse> {
        self.execute_get(
            "/query",
            Some(&[("explain", soql)]),
            "Query explain request failed",
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use crate::client::builder;
    use crate::test_support::{MockAuthenticator, Must};
    use serde_json::json;
    use wiremock::matchers::{header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_explain_success() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());

        let explain_response = json!({
            "plans": [
                {
                    "cardinality": 50,
                    "fields": ["Name"],
                    "leadingOperationType": "Index",
                    "notes": [
                        {
                            "description": "Index used",
                            "fields": ["Name"],
                            "tableEnumOrId": "Account"
                        }
                    ],
                    "relativeCost": 0.33,
                    "sobjectCardinality": 1000,
                    "sobjectType": "Account"
                }
            ]
        });

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/query"))
            .and(query_param(
                "explain",
                "SELECT Id FROM Account WHERE Name = 'Acme'",
            ))
            .and(header("authorization", "Bearer test_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(explain_response))
            .mount(&mock_server)
            .await;

        let client = builder().authenticate(auth).build().await.must();

        let response = client
            .rest()
            .explain("SELECT Id FROM Account WHERE Name = 'Acme'")
            .await
            .must();

        assert_eq!(response.plans.len(), 1);
        let plan = &response.plans[0];
        assert_eq!(plan.leading_operation_type, "Index");
        assert_eq!(plan.sobject_type, "Account");
        assert!((plan.relative_cost - 0.33).abs() < f64::EPSILON);

        let note = plan.notes.as_ref().must().first().must();
        assert_eq!(note.description.as_deref(), Some("Index used"));
    }
}
