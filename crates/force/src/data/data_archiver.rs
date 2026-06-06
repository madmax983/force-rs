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

    /// Exports a SOQL query to a CSV file.
    ///
    /// Requires the `csv` crate/feature. Each returned record will be serialized as a CSV row.
    ///
    /// # Arguments
    ///
    /// * `soql` - The SOQL query string.
    /// * `path` - The file path to write the CSV output.
    ///
    /// # Returns
    ///
    /// Returns the number of records written.
    #[cfg(any(feature = "bulk", feature = "data_utility"))]
    pub async fn export_to_csv<T>(&self, soql: &str, path: impl AsRef<Path>) -> Result<usize>
    where
        T: DeserializeOwned + Serialize + Unpin,
    {
        let mut stream = self.client.rest().query_stream::<T>(soql);
        let file = std::fs::File::create(path)?;
        let mut csv_writer = csv::WriterBuilder::new().from_writer(file);
        let mut count = 0;

        while let Some(record) = stream.next().await? {
            csv_writer
                .serialize(&record)
                .map_err(|e| ForceError::from(SerializationError::Csv(e)))?;
            count += 1;
        }

        csv_writer
            .flush()
            .map_err(|e| ForceError::from(SerializationError::Csv(e.into())))?;
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

    /// Exports a SOQL query to a CSV file, masking sensitive fields.
    ///
    /// # Arguments
    ///
    /// * `sobject_name` - The API name of the SObject being queried (e.g., "Contact").
    /// * `soql` - The SOQL query string.
    /// * `path` - The file path to write the CSV output.
    ///
    /// # Returns
    ///
    /// Returns the number of records written.
    #[cfg(any(feature = "bulk", feature = "data_utility"))]
    pub async fn export_masked_to_csv(
        &self,
        sobject_name: &str,
        soql: &str,
        path: impl AsRef<Path>,
    ) -> Result<usize> {
        let describe = self.client.rest().describe(sobject_name).await?;
        let masker = DataMasker::new(&describe);

        let mut stream = self
            .client
            .rest()
            .query_stream::<crate::types::DynamicSObject>(soql);

        let file = std::fs::File::create(path)?;
        // DynamicSObject serializes fields correctly dynamically for JSON, but might need manual handling for CSV headers.
        // For CSV, it's safer to flatten. `csv` crate handles flat structs.
        let mut csv_writer = csv::WriterBuilder::new().from_writer(file);
        let mut count = 0;
        let mut headers_opt: Option<Vec<String>> = None;

        while let Some(mut record) = stream.next().await? {
            masker.mask_record(&mut record);

            // `csv` crate cannot directly serialize `Map<String, Value>` easily with headers without custom serialization.
            // We manually write string records based on the headers of the first record to ensure alignment.
            if headers_opt.is_none() {
                let headers: Vec<String> = record.fields.keys().cloned().collect();
                csv_writer
                    .write_record(&headers)
                    .map_err(|e| ForceError::from(SerializationError::Csv(e)))?;
                headers_opt = Some(headers);
            }

            if let Some(ref headers) = headers_opt {
                let values: Vec<String> = headers
                    .iter()
                    .map(|header| match record.fields.get(header) {
                        Some(serde_json::Value::String(s)) => s.clone(),
                        Some(serde_json::Value::Null) | None => String::new(),
                        Some(v) => v.to_string(),
                    })
                    .collect();

                csv_writer
                    .write_record(&values)
                    .map_err(|e| ForceError::from(SerializationError::Csv(e)))?;
            }

            count += 1;
        }

        csv_writer
            .flush()
            .map_err(|e| ForceError::from(SerializationError::Csv(e.into())))?;
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
                    "attributes": {"type": "Account", "url": "/services/data/v60.0/sobjects/Account/001xx000000001AAA"},
                    "Id": "001xx000000001AAA",
                    "Name": "Acme"
                },
                {
                    "attributes": {"type": "Account", "url": "/services/data/v60.0/sobjects/Account/001xx000000002AAA"},
                    "Id": "001xx000000002AAA",
                    "Name": "Globex"
                }
            ]
        });

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/query"))
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
            .and(path("/services/data/v60.0/sobjects/Contact/describe"))
            .respond_with(ResponseTemplate::new(200).set_body_json(describe_json))
            .mount(&mock_server)
            .await;

        let query_response = json!({
            "totalSize": 1,
            "done": true,
            "records": [
                {
                    "attributes": {"type": "Contact", "url": "/services/data/v60.0/sobjects/Contact/003xx000000001AAA"},
                    "Id": "003xx000000001AAA",
                    "Name": "Jane Doe",
                    "Email": "jane.doe@example.com"
                }
            ]
        });

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/query"))
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

    #[derive(serde::Deserialize, serde::Serialize)]
    struct TestRecord {
        #[serde(rename = "Id")]
        id: String,
        #[serde(rename = "Name")]
        name: String,
    }

    #[tokio::test]
    #[cfg(any(feature = "bulk", feature = "data_utility"))]
    async fn test_export_to_csv() {
        let mock_server = MockServer::start().await;

        let query_response = json!({
            "totalSize": 2,
            "done": true,
            "records": [
                {
                    "attributes": {"type": "Account", "url": "/services/data/v60.0/sobjects/Account/001xx000000001AAA"},
                    "Id": "001xx000000001AAA",
                    "Name": "Acme"
                },
                {
                    "attributes": {"type": "Account", "url": "/services/data/v60.0/sobjects/Account/001xx000000002AAA"},
                    "Id": "001xx000000002AAA",
                    "Name": "Globex"
                }
            ]
        });

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/query"))
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

        let file_path = env::temp_dir().join(format!("export_{}.csv", std::process::id()));

        let soql = "SELECT Id, Name FROM Account";

        let count = archiver
            .export_to_csv::<TestRecord>(soql, &file_path)
            .await
            .must();

        assert_eq!(count, 2);

        // Verify file contents
        let contents = std::fs::read_to_string(&file_path).must();
        let lines: Vec<&str> = contents.lines().collect();

        // Header + 2 records = 3 lines
        assert_eq!(contents.lines().count(), 3);
        assert_eq!(lines[0], "Id,Name");
        assert_eq!(lines[1], "001xx000000001AAA,Acme");
        assert_eq!(lines[2], "001xx000000002AAA,Globex");

        // Cleanup
        let _ = std::fs::remove_file(file_path);
    }

    #[tokio::test]
    #[cfg(any(feature = "bulk", feature = "data_utility"))]
    async fn test_export_masked_to_csv() {
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
            .and(path("/services/data/v60.0/sobjects/Contact/describe"))
            .respond_with(ResponseTemplate::new(200).set_body_json(describe_json))
            .mount(&mock_server)
            .await;

        let query_response = json!({
            "totalSize": 1,
            "done": true,
            "records": [
                {
                    "attributes": {"type": "Contact", "url": "/services/data/v60.0/sobjects/Contact/003xx000000001AAA"},
                    "Id": "003xx000000001AAA",
                    "Name": "Jane Doe",
                    "Email": "jane.doe@example.com"
                }
            ]
        });

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/query"))
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

        let file_path = env::temp_dir().join(format!("export_masked_{}.csv", std::process::id()));

        let soql = "SELECT Id, Name, Email FROM Contact";
        let count = archiver
            .export_masked_to_csv("Contact", soql, &file_path)
            .await
            .must();

        assert_eq!(count, 1);

        let contents = std::fs::read_to_string(&file_path).must();

        assert_eq!(contents.lines().count(), 2);

        // DynamicSObject serializes its map keys dynamically, so the order of Email/Id/Name isn't strictly guaranteed unless sorted, but we'll check it contains the masked value.
        assert!(contents.contains("Jane Doe"));
        assert!(contents.contains("***@***.***"));

        let _ = std::fs::remove_file(file_path);
    }
}
