//! Field Usage Scanner.
//!
//! This module provides a utility to scan SObjects and determine field usage statistics.
//! It helps identify "zombie fields" (fields that are rarely or never populated).

use crate::api::rest::describe::FieldType;
use crate::client::ForceClient;
use crate::error::Result;
use crate::types::authenticator::Authenticator;
use serde::Deserialize;
use serde_json::Value;

/// Usage statistics for a specific field.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct FieldUsage {
    /// API name of the field.
    pub name: String,
    /// Data type of the field.
    #[serde(rename = "type")]
    pub type_: String,
    /// Number of records where this field is populated (non-null).
    pub populated_count: u64,
    /// Total number of records scanned.
    pub total_count: u64,
    /// Percentage of records where this field is populated (0.0 - 100.0).
    pub percentage: f64,
}

/// Scanner for analyzing field usage.
#[derive(Debug)]
pub struct FieldUsageScanner<'a, A: Authenticator> {
    client: &'a ForceClient<A>,
}

impl<'a, A: Authenticator> FieldUsageScanner<'a, A> {
    /// Creates a new scanner using the provided client.
    #[must_use]
    pub fn new(client: &'a ForceClient<A>) -> Self {
        Self { client }
    }

    /// Scans the specified SObject to determine field usage.
    ///
    /// This method fetches the object definition using the Describe API,
    /// then executes SOQL aggregate queries to count non-null values for each field.
    ///
    /// # Arguments
    ///
    /// * `sobject` - The API name of the SObject to scan (e.g., "Account").
    ///
    /// # Returns
    ///
    /// A list of `FieldUsage` stats for all scanable fields.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The Describe API call fails.
    /// - Any SOQL query fails.
    /// - The response cannot be parsed.
    pub async fn scan(&self, sobject: &str) -> Result<Vec<FieldUsage>> {
        // 1. Describe the object to get fields
        let describe = self.client.rest().describe(sobject).await?;

        // 2. Filter scanable fields
        let scanable_fields: Vec<_> = describe
            .fields
            .iter()
            .filter(|f| is_scanable(&f.type_))
            .collect();

        if scanable_fields.is_empty() {
            return Ok(Vec::new());
        }

        let mut results = Vec::new();

        // 3. Batch fields to avoid SOQL character limits (safe chunk size: 20)
        for chunk in scanable_fields.chunks(20) {
            let usage = self.scan_batch(sobject, chunk).await?;
            results.extend(usage);
        }

        Ok(results)
    }

    async fn scan_batch(
        &self,
        sobject: &str,
        fields: &[&crate::api::rest::describe::FieldDescribe],
    ) -> Result<Vec<FieldUsage>> {
        // Build query: SELECT COUNT(Id) total, COUNT(Field1) f0, COUNT(Field2) f1...
        let mut selects = Vec::new();
        selects.push("COUNT(Id) total".to_string());

        for (i, field) in fields.iter().enumerate() {
            selects.push(format!("COUNT({}) f{}", field.name, i));
        }

        let query = format!("SELECT {} FROM {}", selects.join(", "), sobject);

        // Execute query
        let response = self.client.rest().query::<Value>(&query).await?;

        if response.records.is_empty() {
            // Should not happen for aggregate queries unless table is empty?
            // Actually aggregate query always returns 1 row (unless grouped, which we aren't).
            // Wait, if table is empty, COUNT returns 0 in one row.
            return Ok(fields
                .iter()
                .map(|f| FieldUsage {
                    name: f.name.clone(),
                    type_: format!("{:?}", f.type_),
                    populated_count: 0,
                    total_count: 0,
                    percentage: 0.0,
                })
                .collect());
        }

        let record = &response.records[0];
        let total = record.get("total").and_then(|v| v.as_u64()).unwrap_or(0);

        let mut batch_results = Vec::new();

        for (i, field) in fields.iter().enumerate() {
            let alias = format!("f{}", i);
            let count = record.get(&alias).and_then(|v| v.as_u64()).unwrap_or(0);

            let percentage = if total > 0 {
                (count as f64 / total as f64) * 100.0
            } else {
                0.0
            };

            batch_results.push(FieldUsage {
                name: field.name.clone(),
                type_: format!("{:?}", field.type_),
                populated_count: count,
                total_count: total,
                percentage,
            });
        }

        Ok(batch_results)
    }
}

fn is_scanable(field_type: &FieldType) -> bool {
    !matches!(
        field_type,
        FieldType::Address
            | FieldType::Location
            | FieldType::Base64
            | FieldType::Encryptedstring
            | FieldType::Datacategorygroupreference
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::builder;
    use crate::test_support::{MockAuthenticator, Must};
    use serde_json::json;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_scan_success() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        setup_mock_describe(&mock_server).await;
        setup_mock_query(&mock_server).await;

        let scanner = FieldUsageScanner::new(&client);
        let usage = scanner.scan("Account").await.must();

        assert_eq!(usage.len(), 2);

        let id_usage = &usage[0];
        assert_eq!(id_usage.name, "Id");
        assert_eq!(id_usage.populated_count, 10);
        assert_eq!(id_usage.total_count, 10);
        assert!((id_usage.percentage - 100.0).abs() < f64::EPSILON);

        let name_usage = &usage[1];
        assert_eq!(name_usage.name, "Name");
        assert_eq!(name_usage.populated_count, 5);
        assert_eq!(name_usage.total_count, 10);
        assert!((name_usage.percentage - 50.0).abs() < f64::EPSILON);
    }

    async fn setup_mock_describe(mock_server: &MockServer) {
        // Construct fields separately to avoid recursion limit
        let id_field = json!({
            "name": "Id",
            "type": "id",
            "label": "Account ID",
            "aggregatable": true, "autoNumber": false, "byteLength": 18, "calculated": false,
            "cascadeDelete": false, "caseSensitive": false, "createable": false, "custom": false,
            "defaultedOnCreate": true, "dependentPicklist": false, "deprecatedAndHidden": false,
            "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
            "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
            "idLookup": true, "length": 18, "nameField": false, "namePointing": false, "nillable": false,
            "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
            "referenceTo": [], "restrictedDelete": false, "restrictedPicklist": false, "scale": 0,
            "soapType": "tns:ID", "sortable": true, "unique": false, "updateable": false,
            "writeRequiresMasterRead": false
        });

        let name_field = json!({
            "name": "Name",
            "type": "string",
            "label": "Account Name",
            "aggregatable": true, "autoNumber": false, "byteLength": 18, "calculated": false,
            "cascadeDelete": false, "caseSensitive": false, "createable": false, "custom": false,
            "defaultedOnCreate": true, "dependentPicklist": false, "deprecatedAndHidden": false,
            "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
            "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
            "idLookup": true, "length": 18, "nameField": false, "namePointing": false, "nillable": false,
            "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
            "referenceTo": [], "restrictedDelete": false, "restrictedPicklist": false, "scale": 0,
            "soapType": "xsd:string", "sortable": true, "unique": false, "updateable": false,
            "writeRequiresMasterRead": false
        });

        // Mock Describe
        let describe_json = json!({
            "name": "Account",
            "label": "Account",
            "custom": false,
            "queryable": true,
            "activateable": false,
            "createable": true,
            "customSetting": false,
            "deletable": true,
            "deprecatedAndHidden": false,
            "feedEnabled": true,
            "hasSubtypes": false,
            "isSubtype": false,
            "keyPrefix": "001",
            "labelPlural": "Accounts",
            "layoutable": true,
            "mergeable": true,
            "mruEnabled": true,
            "replicateable": true,
            "retrieveable": true,
            "searchable": true,
            "triggerable": true,
            "undeletable": true,
            "updateable": true,
            "urls": {},
            "childRelationships": [],
            "recordTypeInfos": [],
            "fields": [id_field, name_field]
        });

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/sobjects/Account/describe"))
            .respond_with(ResponseTemplate::new(200).set_body_json(describe_json))
            .mount(mock_server)
            .await;
    }

    async fn setup_mock_query(mock_server: &MockServer) {
        // Mock Query
        // SELECT COUNT(Id) total, COUNT(Id) f0, COUNT(Name) f1 FROM Account
        let query_json = json!({
            "totalSize": 1,
            "done": true,
            "records": [
                {
                    "attributes": {
                        "type": "AggregateResult",
                        "url": "/services/data/v60.0/sobjects/AggregateResult/row0"
                    },
                    "total": 10,
                    "f0": 10, // Id
                    "f1": 5   // Name
                }
            ]
        });

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/query"))
            .and(query_param(
                "q",
                "SELECT COUNT(Id) total, COUNT(Id) f0, COUNT(Name) f1 FROM Account",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(query_json))
            .mount(mock_server)
            .await;
    }
}
