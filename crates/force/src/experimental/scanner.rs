//! Field Usage Scanner - Introspects SObject schema to identify unused fields.
//!
//! This module provides a utility to scan an SObject's fields and determine
//! their usage (population) by querying `COUNT(Field)`.
//!
//! # Example
//!
//! ```ignore
//! use force::experimental::scanner::FieldUsageScanner;
//!
//! let scanner = FieldUsageScanner::new(client);
//! let usage = scanner.scan("Account").await?;
//!
//! for field in usage {
//!     println!("{}: {}% populated", field.name, field.usage_percentage);
//! }
//! ```

use crate::api::rest::describe::FieldType;
use crate::client::ForceClient;
use crate::error::Result;

/// Statistics for a specific field's usage.
#[derive(Debug, Clone)]
pub struct FieldUsage {
    /// API name of the field.
    pub name: String,
    /// User-friendly label.
    pub label: String,
    /// Data type of the field.
    pub field_type: FieldType,
    /// Number of records where this field is not null.
    pub non_null_count: u64,
    /// Total number of records scanned.
    pub total_records: u64,
    /// Percentage of records where this field is populated (0.0 to 100.0).
    pub usage_percentage: f64,
}

/// Scanner for analyzing field usage.
pub struct FieldUsageScanner<A: crate::auth::Authenticator> {
    client: ForceClient<A>,
}

impl<A: crate::auth::Authenticator> FieldUsageScanner<A> {
    /// Creates a new scanner instance.
    pub fn new(client: ForceClient<A>) -> Self {
        Self { client }
    }

    /// Scans the given SObject to determine field usage.
    ///
    /// This method:
    /// 1. Fetches the object's metadata using the Describe API.
    /// 2. Filters out system fields and non-queryable types (Address, Location, Base64, EncryptedString).
    /// 3. Batches fields into groups of 50.
    /// 4. Executes `SELECT COUNT(Id), COUNT(Field)...` queries for each batch.
    /// 5. Aggregates the results.
    ///
    /// # Errors
    ///
    /// Returns an error if the describe or query requests fail.
    pub async fn scan(&self, sobject: &str) -> Result<Vec<FieldUsage>> {
        // 1. Fetch metadata
        let describe = self.client.rest().describe(sobject).await?;

        // 2. Filter queryable fields
        // We exclude compound fields and blobs that can't be aggregated
        let queryable_fields: Vec<_> = describe
            .fields
            .iter()
            .filter(|f| {
                !matches!(
                    f.type_,
                    FieldType::Address
                        | FieldType::Location
                        | FieldType::Base64
                        | FieldType::Encryptedstring
                )
            })
            .collect();

        if queryable_fields.is_empty() {
            return Ok(Vec::new());
        }

        let mut results = Vec::with_capacity(queryable_fields.len());

        // 3. Process in batches to avoid SOQL query complexity limits
        // 50 fields per query is a safe conservative limit
        for chunk in queryable_fields.chunks(50) {
            // Construct query: SELECT COUNT(Id), COUNT(Field1), ...
            let mut select_clause = String::from("COUNT(Id)");
            for field in chunk {
                select_clause.push_str(&format!(", COUNT({})", field.name));
            }

            let soql = format!("SELECT {} FROM {}", select_clause, sobject);

            // 4. Execute query
            // We use serde_json::Value because the structure is dynamic (expr0, expr1...)
            let response = self.client.rest().query::<serde_json::Value>(&soql).await?;

            // 5. Parse results
            // Aggregate queries return one row (unless grouped, which we aren't doing)
            if let Some(record) = response.records.first() {
                // expr0 is COUNT(Id) - total records
                let total_records = record.get("expr0").and_then(|v| v.as_u64()).unwrap_or(0);

                for (i, field) in chunk.iter().enumerate() {
                    // expr1, expr2, ... correspond to fields in the chunk
                    // Note: expr indices are 0-based in SOQL results usually,
                    // but since we started with COUNT(Id) as the first item,
                    // the subsequent counts are expr1, expr2...
                    let expr_key = format!("expr{}", i + 1);

                    let count = record.get(&expr_key).and_then(|v| v.as_u64()).unwrap_or(0);

                    let usage_percentage = if total_records > 0 {
                        (count as f64 / total_records as f64) * 100.0
                    } else {
                        0.0
                    };

                    results.push(FieldUsage {
                        name: field.name.clone(),
                        label: field.label.clone(),
                        field_type: field.type_.clone(),
                        non_null_count: count,
                        total_records,
                        usage_percentage,
                    });
                }
            }
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::builder;
    use crate::test_support::{MockAuthenticator, Must, MustMsg};
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn create_test_client(mock_server_url: String) -> ForceClient<MockAuthenticator> {
        let auth = MockAuthenticator::new("test_token", &mock_server_url);
        builder()
            .authenticate(auth)
            .build()
            .await
            .must_msg("failed to create test client")
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)]
    #[allow(clippy::float_cmp)]
    async fn test_field_usage_scanner() {
        let mock_server = MockServer::start().await;

        // Mock Describe
        // We reuse a minimal valid structure similar to describe.rs tests
        // Split fields to avoid recursion limit
        let f1 = json!({
            "aggregatable": true, "autoNumber": false, "byteLength": 18, "calculated": false,
            "cascadeDelete": false, "caseSensitive": false, "createable": false, "custom": false,
            "defaultedOnCreate": true, "dependentPicklist": false, "deprecatedAndHidden": false,
            "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
            "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
            "idLookup": true, "label": "Account ID", "length": 18, "name": "Id",
            "nameField": false, "namePointing": false, "nillable": false, "permissionable": false,
            "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false, "referenceTo": [],
            "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "tns:ID",
            "sortable": true, "type": "id", "unique": false, "updateable": false,
            "writeRequiresMasterRead": false
        });

        let f2 = json!({
            "aggregatable": true, "autoNumber": false, "byteLength": 255, "calculated": false,
            "cascadeDelete": false, "caseSensitive": false, "createable": true, "custom": false,
            "defaultedOnCreate": false, "dependentPicklist": false, "deprecatedAndHidden": false,
            "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
            "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
            "idLookup": false, "label": "Account Name", "length": 255, "name": "Name",
            "nameField": true, "namePointing": false, "nillable": false, "permissionable": false,
            "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false, "referenceTo": [],
            "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "xsd:string",
            "sortable": true, "type": "string", "unique": false, "updateable": true,
            "writeRequiresMasterRead": false
        });

        let f3 = json!({
            "aggregatable": true, "autoNumber": false, "byteLength": 255, "calculated": false,
            "cascadeDelete": false, "caseSensitive": false, "createable": true, "custom": true,
            "defaultedOnCreate": false, "dependentPicklist": false, "deprecatedAndHidden": false,
            "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
            "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
            "idLookup": false, "label": "Unused Field", "length": 255, "name": "UnusedField__c",
            "nameField": false, "namePointing": false, "nillable": true, "permissionable": false,
            "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false, "referenceTo": [],
            "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "xsd:string",
            "sortable": true, "type": "string", "unique": false, "updateable": true,
            "writeRequiresMasterRead": false
        });

        let fields = json!([f1, f2, f3]);

        let mut describe_json = json!({
            "activateable": false,
            "createable": true,
            "custom": false,
            "customSetting": false,
            "deletable": true,
            "deprecatedAndHidden": false,
            "feedEnabled": true,
            "hasSubtypes": false,
            "isSubtype": false,
            "keyPrefix": "001",
            "label": "Account",
            "labelPlural": "Accounts",
            "layoutable": true,
            "mergeable": true,
            "mruEnabled": true,
            "name": "Account",
            "queryable": true,
            "replicateable": true,
            "retrieveable": true,
            "searchable": true,
            "triggerable": true,
            "undeletable": true,
            "updateable": true,
            "urls": {
                "sobject": "/services/data/v60.0/sobjects/Account"
            },
            "childRelationships": [],
            "recordTypeInfos": []
        });

        describe_json
            .as_object_mut()
            .must()
            .insert("fields".to_string(), fields);

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/sobjects/Account/describe"))
            .respond_with(ResponseTemplate::new(200).set_body_json(describe_json))
            .mount(&mock_server)
            .await;

        // Mock Query
        // SELECT COUNT(Id), COUNT(Id), COUNT(Name), COUNT(UnusedField__c) FROM Account
        let query_response = json!({
            "totalSize": 1,
            "done": true,
            "records": [
                {
                    "attributes": { "type": "AggregateResult" },
                    "expr0": 100, // Total records (COUNT(Id))
                    "expr1": 100, // COUNT(Id)
                    "expr2": 90,  // COUNT(Name)
                    "expr3": 0    // COUNT(UnusedField__c)
                }
            ]
        });

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/query"))
            .respond_with(ResponseTemplate::new(200).set_body_json(query_response))
            .mount(&mock_server)
            .await;

        let client = create_test_client(mock_server.uri()).await;
        let scanner = FieldUsageScanner::new(client);
        let usage = scanner.scan("Account").await.must();

        assert_eq!(usage.len(), 3);

        // Find Id field
        let id_field = usage.iter().find(|f| f.name == "Id").must();
        assert_eq!(id_field.total_records, 100);
        assert_eq!(id_field.non_null_count, 100);
        assert_eq!(id_field.usage_percentage, 100.0);

        // Find Name field
        let name_field = usage.iter().find(|f| f.name == "Name").must();
        assert_eq!(name_field.non_null_count, 90);
        assert_eq!(name_field.usage_percentage, 90.0);

        // Find Unused field
        let unused_field = usage.iter().find(|f| f.name == "UnusedField__c").must();
        assert_eq!(unused_field.non_null_count, 0);
        assert_eq!(unused_field.usage_percentage, 0.0);
    }
}
