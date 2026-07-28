//! Data Profiler utility for analyzing Salesforce SObject data shape.
//!
//! This module provides the `DataProfiler` utility to dynamically construct
//! aggregate SOQL queries based on SObject Describe metadata to compute field fill-rates.

use crate::api::RestOperation;
use crate::auth::Authenticator;
use crate::client::ForceClient;
use crate::error::Result;
use crate::types::describe::SObjectDescribe;
use serde_json::Value;
use std::fmt::{Display, Formatter};

/// Profile of a single field's data completeness.
#[derive(Debug, Clone)]
pub struct FieldProfile {
    /// The API name of the field.
    pub name: String,
    /// The total number of non-null values.
    pub populated_count: usize,
    /// The percentage of records where this field is populated (0.0 to 100.0).
    pub fill_rate: f64,
}

/// Overall profile of an SObject's data shape.
#[derive(Debug, Clone)]
pub struct ObjectProfile {
    /// The SObject API name.
    pub sobject: String,
    /// Total number of records analyzed.
    pub total_records: usize,
    /// Profiles for individual fields.
    pub fields: Vec<FieldProfile>,
}

impl Display for ObjectProfile {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "📊 Data Profile: {}", self.sobject)?;
        writeln!(f, "Total Records: {}", self.total_records)?;
        writeln!(f, "--------------------------------------------------")?;
        for field in &self.fields {
            writeln!(
                f,
                "{:<30} | Populated: {:<8} | Fill Rate: {:.2}%",
                field.name, field.populated_count, field.fill_rate
            )?;
        }
        Ok(())
    }
}

/// Utility for dynamically profiling Salesforce data using aggregate SOQL.
#[derive(Debug)]
pub struct DataProfiler<'a, A: Authenticator> {
    client: &'a ForceClient<A>,
}

impl<'a, A: Authenticator> DataProfiler<'a, A> {
    /// Creates a new data profiler.
    #[must_use]
    pub fn new(client: &'a ForceClient<A>) -> Self {
        Self { client }
    }

    /// Profiles the given SObject by dynamically querying counts for all fields.
    pub async fn profile_object(&self, describe: &SObjectDescribe) -> Result<ObjectProfile> {
        let mut select_clauses = vec!["COUNT(Id) total".to_string()];

        // Build aggregate COUNT(field) for each field
        for field in &describe.fields {
            if field.aggregatable {
                select_clauses.push(format!("COUNT({}) {}", field.name, field.name));
            }
        }

        let soql = format!(
            "SELECT {} FROM {}",
            select_clauses.join(", "),
            describe.name
        );

        // Execute the aggregate query
        let results = self.client.rest().query::<Value>(&soql).await?;

        let record = results.records.first().unwrap_or(&Value::Null);

        let total_records =
            usize::try_from(record.get("total").and_then(Value::as_u64).unwrap_or(0)).unwrap_or(0);

        let mut fields = Vec::new();

        for field in &describe.fields {
            if !field.aggregatable {
                continue;
            }

            let populated_count =
                usize::try_from(record.get(&field.name).and_then(Value::as_u64).unwrap_or(0))
                    .unwrap_or(0);

            let fill_rate = if total_records > 0 {
                (populated_count as f64 / total_records as f64) * 100.0
            } else {
                0.0
            };

            fields.push(FieldProfile {
                name: field.name.clone(),
                populated_count,
                fill_rate,
            });
        }

        // Sort by fill rate descending
        fields.sort_by(|a, b| {
            b.fill_rate
                .partial_cmp(&a.fill_rate)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(ObjectProfile {
            sobject: describe.name.clone(),
            total_records,
            fields,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::ForceClientBuilder;
    use crate::test_utils::mock_auth::MockAuthenticator;
    use crate::test_utils::must::Must;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn test_profile_formatting() {
        let profile = ObjectProfile {
            sobject: "Account".to_string(),
            total_records: 100,
            fields: vec![
                FieldProfile {
                    name: "Id".to_string(),
                    populated_count: 100,
                    fill_rate: 100.0,
                },
                FieldProfile {
                    name: "Website".to_string(),
                    populated_count: 50,
                    fill_rate: 50.0,
                },
            ],
        };

        let output = format!("{}", profile);
        assert!(output.contains("📊 Data Profile: Account"));
        assert!(output.contains("Total Records: 100"));
        assert!(
            output.contains(
                "Id                             | Populated: 100      | Fill Rate: 100.00%"
            )
        );
        assert!(
            output.contains(
                "Website                        | Populated: 50       | Fill Rate: 50.00%"
            )
        );
    }

    #[tokio::test]
    async fn test_profile_object() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("token", &mock_server.uri());
        let client = ForceClientBuilder::new()
            .authenticate(auth)
            .build()
            .await
            .must();

        let describe_json = r#"{
            "name": "Account",
            "label": "Account",
            "custom": false,
            "queryable": true,
            "activateable": false, "createable": true, "customSetting": false, "deletable": true,
            "deprecatedAndHidden": false, "feedEnabled": true, "hasSubtypes": false,
            "isSubtype": false, "labelPlural": "Accounts", "layoutable": true,
            "urls": {}, "childRelationships": [], "recordTypeInfos": [],
            "mergeable": true, "mruEnabled": true, "replicateable": true, "retrieveable": true,
            "searchable": true, "triggerable": true, "undeletable": true, "updateable": true,
            "fields": [
                {
                    "name": "Id",
                    "type": "id",
                    "label": "Account ID",
                    "referenceTo": [], "aggregatable": true, "autoNumber": false, "byteLength": 18, "calculated": false,
                    "cascadeDelete": false, "caseSensitive": false, "createable": false, "custom": false,
                    "defaultedOnCreate": true, "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 0, "precision": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": true, "length": 18, "nameField": false, "namePointing": false, "nillable": false,
                    "permissionable": false, "polymorphicForeignKey": false, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "searchPrefilterable": false, "sortable": true,
                    "soapType": "tns:ID", "unique": false, "writeRequiresMasterRead": false, "updateable": false
                },
                {
                    "name": "Name",
                    "type": "string",
                    "label": "Account Name",
                    "referenceTo": [], "aggregatable": true, "autoNumber": false, "byteLength": 255, "calculated": false,
                    "cascadeDelete": false, "caseSensitive": false, "createable": true, "custom": false,
                    "defaultedOnCreate": false, "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 0, "precision": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": false, "length": 255, "nameField": true, "namePointing": false, "nillable": false,
                    "permissionable": false, "polymorphicForeignKey": false, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "searchPrefilterable": false, "sortable": true,
                    "soapType": "xsd:string", "unique": false, "writeRequiresMasterRead": false, "updateable": true
                },
                {
                    "name": "Description",
                    "type": "textarea",
                    "label": "Description",
                    "referenceTo": [], "aggregatable": false, "autoNumber": false, "byteLength": 255, "calculated": false,
                    "cascadeDelete": false, "caseSensitive": false, "createable": false, "custom": true,
                    "defaultedOnCreate": false, "dependentPicklist": false, "deprecatedAndHidden": true,
                    "digits": 0, "precision": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": false, "groupable": false, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": false, "length": 255, "nameField": false, "namePointing": false, "nillable": true,
                    "permissionable": false, "polymorphicForeignKey": false, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "searchPrefilterable": false, "sortable": false,
                    "soapType": "xsd:string", "unique": false, "writeRequiresMasterRead": false, "updateable": false
                }
            ]
        }"#;

        let describe: SObjectDescribe = serde_json::from_str(describe_json).must();

        let expected_soql = "SELECT COUNT(Id) total, COUNT(Id) Id, COUNT(Name) Name FROM Account";

        let query_response = serde_json::json!({
            "totalSize": 1,
            "done": true,
            "records": [
                {
                    "attributes": { "type": "AggregateResult" },
                    "total": 100,
                    "Id": 100,
                    "Name": 95
                }
            ]
        });

        Mock::given(method("GET"))
            .and(path("/services/data/v67.0/query"))
            .and(query_param("q", expected_soql))
            .respond_with(ResponseTemplate::new(200).set_body_json(query_response))
            .mount(&mock_server)
            .await;

        let profiler = DataProfiler::new(&client);
        let profile = profiler.profile_object(&describe).await.must();

        assert_eq!(profile.sobject, "Account");
        assert_eq!(profile.total_records, 100);
        assert_eq!(profile.fields.len(), 2);

        assert_eq!(profile.fields[0].name, "Id");
        assert_eq!(profile.fields[0].fill_rate, 100.0);

        assert_eq!(profile.fields[1].name, "Name");
        assert_eq!(profile.fields[1].fill_rate, 95.0);
    }
}
