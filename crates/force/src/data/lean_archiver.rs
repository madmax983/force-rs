//! Lean Archiver - Smart data extraction.
//!
//! This module provides the `LeanArchiver` utility, which combines `FieldUsageScanner`
//! and `DataArchiver`. It analyzes an SObject to find which fields are actually used
//! (populated in at least one record) and dynamically constructs a SOQL query to export
//! only the populated data. This reduces bandwidth, avoids querying thousands of empty
//! fields, and creates a "lean" export.
//!
//! # The Spark
//! We have `DataArchiver` for exporting data and `FieldUsageScanner` for finding
//! zombie fields. If we combine them, we get an intelligent extractor that only
//! exports what matters!

use crate::auth::Authenticator;
use crate::client::ForceClient;
use crate::data::DataArchiver;
use crate::error::Result;
use crate::schema::FieldUsageScanner;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::path::Path;

/// Intelligent data archiver that only extracts populated fields.
#[derive(Debug)]
pub struct LeanArchiver<'a, A: Authenticator> {
    client: &'a ForceClient<A>,
}

impl<'a, A: Authenticator> LeanArchiver<'a, A> {
    /// Creates a new LeanArchiver.
    #[must_use]
    pub fn new(client: &'a ForceClient<A>) -> Self {
        Self { client }
    }

    /// Exports only the populated fields of an SObject to a JSON Lines (JSONL) file.
    ///
    /// # Arguments
    ///
    /// * `sobject_name` - The API name of the SObject (e.g., "Account").
    /// * `path` - The file path to write the JSONL output.
    /// * `usage_threshold_percent` - The minimum population percentage required to include a field (e.g., 0.0 for any usage).
    pub async fn export_lean_to_jsonl<T>(
        &self,
        sobject_name: &str,
        path: impl AsRef<Path>,
        usage_threshold_percent: f64,
    ) -> Result<usize>
    where
        T: DeserializeOwned + Serialize + Unpin,
    {
        // 1. Scan for used fields
        let scanner = FieldUsageScanner::new(self.client);
        let usages = scanner.scan(sobject_name).await?;

        // 2. Filter fields by usage
        let mut active_fields: Vec<String> = usages
            .into_iter()
            .filter(|u| u.percentage > usage_threshold_percent)
            .map(|u| u.name)
            .collect();

        // Always include Id
        if !active_fields.iter().any(|f| f.eq_ignore_ascii_case("Id")) {
            active_fields.insert(0, "Id".to_string());
        }

        // 3. Build the SOQL query
        let fields_str = active_fields.join(", ");
        let soql = format!("SELECT {} FROM {}", fields_str, sobject_name);

        // 4. Archive using DataArchiver
        let archiver = DataArchiver::new(self.client);
        archiver.export_to_jsonl::<T>(&soql, path).await
    }
}

#[cfg(test)]
#[cfg(feature = "mock")]
mod tests {
    use super::*;
    use crate::client::builder;
    use crate::test_utils::mock_auth::MockAuthenticator;
    use crate::test_utils::must::Must;
    use crate::types::DynamicSObject;
    use tempfile::NamedTempFile;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    #[allow(clippy::too_many_lines)]
    async fn test_lean_export() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        // 1. Mock Describe
        let describe_json = serde_json::json!({
            "name": "Account",
            "label": "Account",
            "custom": false,
            "queryable": true,
            "activateable": false, "createable": true, "customSetting": false, "deletable": true,
            "deprecatedAndHidden": false, "feedEnabled": true, "hasSubtypes": false,
            "isSubtype": false, "keyPrefix": "001", "labelPlural": "Accounts", "layoutable": true,
            "mergeable": true, "mruEnabled": true, "replicateable": true, "retrieveable": true,
            "searchable": true, "triggerable": true, "undeletable": true, "updateable": true,
            "urls": {}, "childRelationships": [], "recordTypeInfos": [],
            "fields": [
                {
                    "name": "Id", "type": "id", "label": "Account ID",
                    "referenceTo": [],
                    "aggregatable": true, "autoNumber": false, "byteLength": 18, "calculated": false,
                    "cascadeDelete": false, "caseSensitive": false, "createable": false, "custom": false,
                    "defaultedOnCreate": true, "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": true, "length": 18, "nameField": false, "namePointing": false, "nillable": false,
                    "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "tns:ID",
                    "sortable": true, "unique": false, "updateable": false, "writeRequiresMasterRead": false
                },
                {
                    "name": "Name", "type": "string", "label": "Account Name",
                    "referenceTo": [],
                    "aggregatable": true, "autoNumber": false, "byteLength": 18, "calculated": false,
                    "cascadeDelete": false, "caseSensitive": false, "createable": false, "custom": false,
                    "defaultedOnCreate": true, "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": true, "length": 18, "nameField": false, "namePointing": false, "nillable": false,
                    "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "xsd:string",
                    "sortable": true, "unique": false, "updateable": false, "writeRequiresMasterRead": false
                },
                {
                    "name": "EmptyField__c", "type": "string", "label": "Empty Field",
                    "referenceTo": [],
                    "aggregatable": true, "autoNumber": false, "byteLength": 18, "calculated": false,
                    "cascadeDelete": false, "caseSensitive": false, "createable": false, "custom": false,
                    "defaultedOnCreate": true, "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": true, "length": 18, "nameField": false, "namePointing": false, "nillable": false,
                    "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "xsd:string",
                    "sortable": true, "unique": false, "updateable": false, "writeRequiresMasterRead": false
                }
            ]
        });

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/sobjects/Account/describe"))
            .respond_with(ResponseTemplate::new(200).set_body_json(describe_json))
            .mount(&mock_server)
            .await;

        // 2. Mock Count Query from FieldUsageScanner
        let count_response = serde_json::json!({
            "totalSize": 1, "done": true, "records": [{"total": 100, "f0": 100, "f1": 50, "f2": 0}]
        });
        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/query"))
            .and(query_param("q", "SELECT COUNT(Id) total, COUNT(Id) f0, COUNT(Name) f1, COUNT(EmptyField__c) f2 FROM Account"))
            .respond_with(ResponseTemplate::new(200).set_body_json(count_response))
            .mount(&mock_server)
            .await;

        // 3. Mock Data Query (SELECT Id, Name FROM Account)
        let data_response = serde_json::json!({
            "totalSize": 1, "done": true, "records": [
                {
                    "attributes": {
                        "type": "Account",
                        "url": "/services/data/v60.0/sobjects/Account/001000000000001AAA"
                    },
                    "Id": "001000000000001AAA",
                    "Name": "Test Account"
                }
            ]
        });
        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/query"))
            .and(query_param("q", "SELECT Id, Name FROM Account"))
            .respond_with(ResponseTemplate::new(200).set_body_json(data_response))
            .mount(&mock_server)
            .await;

        let archiver = LeanArchiver::new(&client);
        let temp_file = NamedTempFile::new().must();

        let count = archiver
            .export_lean_to_jsonl::<DynamicSObject>("Account", temp_file.path(), 0.0)
            .await
            .must();

        assert_eq!(count, 1);

        let contents = std::fs::read_to_string(temp_file.path()).must();
        assert!(contents.contains("Test Account"));
    }
}
