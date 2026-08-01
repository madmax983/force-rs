//! Data archiver for Salesforce.
//!
//! Provides `DataArchiver`, a utility for seamlessly exporting Salesforce data
//! to local disk formats (JSONL, CSV).

use super::DataMasker;
use crate::api::rest_operation::RestOperation;
use crate::auth::Authenticator;
use crate::client::ForceClient;
use crate::error::{ForceError, Result, SerializationError};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::path::Path;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

/// Utility for exporting query results to disk.
#[derive(Debug)]
pub struct DataArchiver<'a, A: Authenticator> {
    client: &'a ForceClient<A>,
}

impl<'a, A: Authenticator> DataArchiver<'a, A> {
    /// Creates a new data archiver.
    ///
    /// # Arguments
    ///
    /// * `client` - The Force client.
    #[must_use]
    pub fn new(client: &'a ForceClient<A>) -> Self {
        Self { client }
    }

    /// Exports a SOQL query to a JSON Lines (JSONL) file.
    ///
    /// Each returned record will be serialized as a single JSON object per line.
    ///
    /// # Arguments
    ///
    /// * `soql` - The SOQL query string.
    /// * `path` - The file path to write the JSONL output.
    ///
    /// # Returns
    ///
    /// Returns the number of records written.
    pub async fn export_to_jsonl<T>(&self, soql: &str, path: impl AsRef<Path>) -> Result<usize>
    where
        T: DeserializeOwned + Serialize + Unpin,
    {
        if path.as_ref().is_absolute()
            || path
                .as_ref()
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return Err(crate::error::ForceError::InvalidInput(
                "Path traversal detected".to_string(),
            ));
        }
        let mut stream = self.client.rest().query_stream::<T>(soql);
        let mut file = File::create(path).await?;
        let mut count = 0;

        while let Some(record) = stream.next().await? {
            let json = serde_json::to_string(&record)
                .map_err(|e| ForceError::from(SerializationError::from(e)))?;

            file.write_all(json.as_bytes()).await?;
            file.write_all(b"\n").await?;

            count += 1;
        }

        file.flush().await?;
        Ok(count)
    }

    /// Exports a SOQL query to a JSON Lines (JSONL) file, masking sensitive fields.
    ///
    /// This method fetches the `SObjectDescribe` metadata for the given object
    /// and uses `DataMasker` to automatically redact PII (like Emails, SSNs, etc)
    /// before writing each record to the output file.
    ///
    /// # Arguments
    ///
    /// * `sobject_name` - The API name of the SObject being queried (e.g., "Contact").
    /// * `soql` - The SOQL query string.
    /// * `path` - The file path to write the JSONL output.
    ///
    /// # Returns
    ///
    /// Returns the number of records written.
    pub async fn export_masked_to_jsonl(
        &self,
        sobject_name: &str,
        soql: &str,
        path: impl AsRef<Path>,
    ) -> Result<usize> {
        if path.as_ref().is_absolute()
            || path
                .as_ref()
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return Err(crate::error::ForceError::InvalidInput(
                "Path traversal detected".to_string(),
            ));
        }
        let describe = self.client.rest().describe(sobject_name).await?;
        let masker = DataMasker::new(&describe);

        let mut stream = self
            .client
            .rest()
            .query_stream::<crate::types::DynamicSObject>(soql);

        let mut file = File::create(path).await?;
        let mut count = 0;

        while let Some(mut record) = stream.next().await? {
            masker.mask_record(&mut record);

            let json = serde_json::to_string(&record)
                .map_err(|e| ForceError::from(SerializationError::from(e)))?;

            file.write_all(json.as_bytes()).await?;
            file.write_all(b"\n").await?;

            count += 1;
        }

        file.flush().await?;
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::ForceClientBuilder;
    use crate::test_utils::mock_auth::MockAuthenticator;
    use crate::test_utils::must::Must;
    use serde_json::json;
    use std::env;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_export_to_jsonl() {
        let mock_server = MockServer::start().await;

        let query_response = json!({
            "totalSize": 2,
            "done": true,
            "records": [
                {
                    "attributes": {"type": "Account", "url": "/services/data/v67.0/sobjects/Account/001xx000000001AAA"},
                    "Id": "001xx000000001AAA",
                    "Name": "Acme"
                },
                {
                    "attributes": {"type": "Account", "url": "/services/data/v67.0/sobjects/Account/001xx000000002AAA"},
                    "Id": "001xx000000002AAA",
                    "Name": "Globex"
                }
            ]
        });

        Mock::given(method("GET"))
            .and(path("/services/data/v67.0/query"))
            .respond_with(ResponseTemplate::new(200).set_body_json(query_response))
            .mount(&mock_server)
            .await;

        let auth = MockAuthenticator::new("token", &mock_server.uri());
        let client = ForceClientBuilder::new()
            .authenticate(auth)
            .build()
            .await
            .must();
        let archiver = DataArchiver::new(&client);

        let file_path = env::temp_dir().join(format!("export_{}.jsonl", std::process::id()));

        let soql = "SELECT Id, Name FROM Account";
        let count = archiver
            .export_to_jsonl::<serde_json::Value>(soql, &file_path)
            .await
            .must();

        assert_eq!(count, 2);

        // Verify file contents
        let contents = std::fs::read_to_string(&file_path).must();
        let lines: Vec<&str> = contents.lines().collect();

        assert_eq!(lines.len(), 2);

        let record1: serde_json::Value = serde_json::from_str(lines[0]).must();
        assert_eq!(record1["Name"], "Acme");

        let record2: serde_json::Value = serde_json::from_str(lines[1]).must();
        assert_eq!(record2["Name"], "Globex");

        // Cleanup
        let _ = std::fs::remove_file(file_path);
    }

    #[tokio::test]
    async fn test_export_masked_to_jsonl() {
        let mock_server = MockServer::start().await;

        let describe_json: serde_json::Value = serde_json::from_str(r#"{
            "name": "Contact",
            "label": "Contact",
            "custom": false,
            "queryable": true,
            "activateable": false, "createable": true, "customSetting": false, "deletable": true,
            "deprecatedAndHidden": false, "feedEnabled": true, "hasSubtypes": false,
            "isSubtype": false, "keyPrefix": "003", "labelPlural": "Contacts", "layoutable": true,
            "mergeable": true, "mruEnabled": true, "replicateable": true, "retrieveable": true,
            "searchable": true, "triggerable": true, "undeletable": true, "updateable": true,
            "urls": {}, "childRelationships": [], "recordTypeInfos": [],
            "fields": [
                {
                    "name": "Name", "type": "string", "label": "Name", "createable": true,
                    "autoNumber": false, "calculated": false,
                    "aggregatable": true, "byteLength": 18,
                    "cascadeDelete": false, "caseSensitive": false, "custom": false,
                    "defaultedOnCreate": true, "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": true, "length": 18, "nameField": false, "namePointing": false, "nillable": false,
                    "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "xsd:string",
                    "sortable": true, "unique": false, "updateable": false, "writeRequiresMasterRead": false,
                    "referenceTo": []
                },
                {
                    "name": "Email", "type": "email", "label": "Email", "createable": true,
                    "autoNumber": false, "calculated": false,
                    "aggregatable": true, "byteLength": 18,
                    "cascadeDelete": false, "caseSensitive": false, "custom": false,
                    "defaultedOnCreate": true, "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": true, "length": 18, "nameField": false, "namePointing": false, "nillable": false,
                    "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "xsd:string",
                    "sortable": true, "unique": false, "updateable": false, "writeRequiresMasterRead": false,
                    "referenceTo": []
                }
            ]
        }"#).must();

        Mock::given(method("GET"))
            .and(path("/services/data/v67.0/sobjects/Contact/describe"))
            .respond_with(ResponseTemplate::new(200).set_body_json(describe_json))
            .mount(&mock_server)
            .await;

        let query_response = json!({
            "totalSize": 1,
            "done": true,
            "records": [
                {
                    "attributes": {"type": "Contact", "url": "/services/data/v67.0/sobjects/Contact/003xx000000001AAA"},
                    "Id": "003xx000000001AAA",
                    "Name": "Jane Doe",
                    "Email": "jane.doe@example.com"
                }
            ]
        });

        Mock::given(method("GET"))
            .and(path("/services/data/v67.0/query"))
            .respond_with(ResponseTemplate::new(200).set_body_json(query_response))
            .mount(&mock_server)
            .await;

        let auth = MockAuthenticator::new("token", &mock_server.uri());
        let client = ForceClientBuilder::new()
            .authenticate(auth)
            .build()
            .await
            .must();
        let archiver = DataArchiver::new(&client);

        let file_path = env::temp_dir().join(format!("export_masked_{}.jsonl", std::process::id()));

        let soql = "SELECT Id, Name, Email FROM Contact";
        let count = archiver
            .export_masked_to_jsonl("Contact", soql, &file_path)
            .await
            .must();

        assert_eq!(count, 1);

        let contents = std::fs::read_to_string(&file_path).must();
        let lines: Vec<&str> = contents.lines().collect();

        assert_eq!(lines.len(), 1);

        let record: serde_json::Value = serde_json::from_str(lines[0]).must();
        assert_eq!(record["Name"], "Jane Doe");
        assert_eq!(record["Email"], "***@***.***"); // Should be masked!

        let _ = std::fs::remove_file(file_path);
    }

    #[tokio::test]
    async fn test_export_to_jsonl_path_traversal() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("token", &mock_server.uri());
        let client = ForceClientBuilder::new()
            .authenticate(auth)
            .build()
            .await
            .must();
        let archiver = DataArchiver::new(&client);

        let result1 = archiver
            .export_to_jsonl::<serde_json::Value>("SELECT Id FROM Account", "../../etc/passwd")
            .await;

        if let Err(crate::error::ForceError::InvalidInput(msg)) = result1 {
            assert_eq!(msg, "Path traversal detected");
        } else {
            panic!("Expected InvalidInput error due to path traversal");
        }

        let result2 = archiver
            .export_to_jsonl::<serde_json::Value>("SELECT Id FROM Account", "/etc/passwd")
            .await;

        if let Err(crate::error::ForceError::InvalidInput(msg)) = result2 {
            assert_eq!(msg, "Path traversal detected");
        } else {
            panic!("Expected InvalidInput error due to absolute path");
        }
    }

    #[tokio::test]
    async fn test_export_masked_to_jsonl_path_traversal() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("token", &mock_server.uri());
        let client = ForceClientBuilder::new()
            .authenticate(auth)
            .build()
            .await
            .must();
        let archiver = DataArchiver::new(&client);

        let result1 = archiver
            .export_masked_to_jsonl("Account", "SELECT Id FROM Account", "../../etc/passwd")
            .await;

        if let Err(crate::error::ForceError::InvalidInput(msg)) = result1 {
            assert_eq!(msg, "Path traversal detected");
        } else {
            panic!("Expected InvalidInput error due to path traversal");
        }

        let result2 = archiver
            .export_masked_to_jsonl("Account", "SELECT Id FROM Account", "/etc/passwd")
            .await;

        if let Err(crate::error::ForceError::InvalidInput(msg)) = result2 {
            assert_eq!(msg, "Path traversal detected");
        } else {
            panic!("Expected InvalidInput error due to absolute path");
        }
    }
}
