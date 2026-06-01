//! Data cleanser for Salesforce.
//!
//! Provides `DataCleanser`, a utility for validating and physically separating
//! JSONL data files into clean and quarantined datasets using an object's schema.

use super::data_validator::DataValidator;
use crate::error::{ForceError, Result, SerializationError};
use crate::types::DynamicSObject;
use crate::types::describe::SObjectDescribe;
use std::path::Path;
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

/// Statistics returned after a cleansing operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanserStats {
    /// Total number of records processed.
    pub total_processed: usize,
    /// Number of records written to the clean file.
    pub clean_count: usize,
    /// Number of records written to the quarantine file.
    pub quarantine_count: usize,
}

/// Utility for validating and routing JSONL records locally.
#[derive(Debug)]
pub struct DataCleanser<'a> {
    describe: &'a SObjectDescribe,
}

impl<'a> DataCleanser<'a> {
    /// Creates a new data cleanser for the specified schema.
    ///
    /// # Arguments
    ///
    /// * `describe` - The schema describing the expected object structure.
    #[must_use]
    pub fn new(describe: &'a SObjectDescribe) -> Self {
        Self { describe }
    }

    /// Reads a JSONL file, validates each record against the schema,
    /// and routes valid records to `clean_path` and invalid records to `quarantine_path`.
    ///
    /// # Arguments
    ///
    /// * `input_path` - The source JSONL file path.
    /// * `clean_path` - Output path for records that pass validation.
    /// * `quarantine_path` - Output path for records that fail validation or parsing.
    ///
    /// # Returns
    ///
    /// Returns `CleanserStats` containing the counts of processed, clean, and quarantined records.
    pub async fn cleanse(
        &self,
        input_path: impl AsRef<Path>,
        clean_path: impl AsRef<Path>,
        quarantine_path: impl AsRef<Path>,
    ) -> Result<CleanserStats> {
        let input_file = File::open(input_path).await?;
        let mut clean_file = File::create(clean_path).await?;
        let mut quarantine_file = File::create(quarantine_path).await?;

        let validator = DataValidator::new(self.describe);
        let mut reader = BufReader::new(input_file).lines();

        let mut stats = CleanserStats {
            total_processed: 0,
            clean_count: 0,
            quarantine_count: 0,
        };

        while let Some(line) = reader.next_line().await? {
            stats.total_processed += 1;

            // Fast route empty lines
            if line.trim().is_empty() {
                continue;
            }

            match serde_json::from_str::<DynamicSObject>(&line) {
                Ok(record) => {
                    if let Err(errors) = validator.validate(&record) {
                        // Routing to quarantine because validation failed.
                        // We wrap it in a JSON object with the validation errors for debugging.
                        let quarantine_record = serde_json::json!({
                            "original": record,
                            "errors": errors.iter().map(|e| e.to_string()).collect::<Vec<String>>()
                        });
                        let quarantine_json = serde_json::to_string(&quarantine_record)
                            .map_err(|e| ForceError::from(SerializationError::from(e)))?;

                        quarantine_file
                            .write_all(quarantine_json.as_bytes())
                            .await?;
                        quarantine_file.write_all(b"\n").await?;
                        stats.quarantine_count += 1;
                    } else {
                        // Routing to clean
                        clean_file.write_all(line.as_bytes()).await?;
                        clean_file.write_all(b"\n").await?;
                        stats.clean_count += 1;
                    }
                }
                Err(err) => {
                    // Parsing failed, it's malformed JSON.
                    let quarantine_record = serde_json::json!({
                        "raw_line": line,
                        "parse_error": err.to_string()
                    });
                    let quarantine_json = serde_json::to_string(&quarantine_record)
                        .map_err(|e| ForceError::from(SerializationError::from(e)))?;

                    quarantine_file
                        .write_all(quarantine_json.as_bytes())
                        .await?;
                    quarantine_file.write_all(b"\n").await?;
                    stats.quarantine_count += 1;
                }
            }
        }

        clean_file.flush().await?;
        quarantine_file.flush().await?;

        Ok(stats)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::must::Must;
    use std::env;

    #[tokio::test]
    async fn test_cleansing_flow() {
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
                    "name": "LastName", "type": "string", "label": "Last Name", "createable": true,
                    "autoNumber": false, "calculated": false,
                    "aggregatable": true, "byteLength": 80,
                    "cascadeDelete": false, "caseSensitive": false, "custom": false,
                    "defaultedOnCreate": false, "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": true, "length": 80, "nameField": true, "namePointing": false, "nillable": false,
                    "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "xsd:string",
                    "sortable": true, "unique": false, "updateable": true, "writeRequiresMasterRead": false,
                    "referenceTo": []
                },
                {
                    "name": "Email", "type": "email", "label": "Email", "createable": true,
                    "autoNumber": false, "calculated": false,
                    "aggregatable": true, "byteLength": 80,
                    "cascadeDelete": false, "caseSensitive": false, "custom": false,
                    "defaultedOnCreate": true, "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": true, "length": 80, "nameField": false, "namePointing": false, "nillable": true,
                    "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "xsd:string",
                    "sortable": true, "unique": false, "updateable": true, "writeRequiresMasterRead": false,
                    "referenceTo": []
                }
            ]
        }"#).must();

        let describe: SObjectDescribe = serde_json::from_value(describe_json).must();
        let cleanser = DataCleanser::new(&describe);

        let test_dir = env::temp_dir();
        let pid = std::process::id();
        let input_path = test_dir.join(format!("input_{}.jsonl", pid));
        let clean_path = test_dir.join(format!("clean_{}.jsonl", pid));
        let quarantine_path = test_dir.join(format!("quarantine_{}.jsonl", pid));

        // 1. Clean record
        // 2. Missing required field (LastName)
        // 3. Malformed JSON
        let input_data = [
            r#"{"attributes":{"type":"Contact","url":""},"LastName":"Doe","Email":"doe@test.com"}"#,
            r#"{"attributes":{"type":"Contact","url":""},"Email":"smith@test.com"}"#,
            r#"{"attributes":{"type":"Contact","url":""},"LastName":"Broken"#, // Missing closing brace
        ]
        .join("\n");

        std::fs::write(&input_path, input_data).must();

        let stats = cleanser
            .cleanse(&input_path, &clean_path, &quarantine_path)
            .await
            .must();

        assert_eq!(stats.total_processed, 3);
        assert_eq!(stats.clean_count, 1);
        assert_eq!(stats.quarantine_count, 2);

        let clean_content = std::fs::read_to_string(&clean_path).must();
        let clean_lines: Vec<&str> = clean_content.lines().collect();
        assert_eq!(clean_lines.len(), 1);
        assert!(clean_lines[0].contains("Doe"));

        let quarantine_content = std::fs::read_to_string(&quarantine_path).must();
        let quarantine_lines: Vec<&str> = quarantine_content.lines().collect();
        assert_eq!(quarantine_lines.len(), 2);

        let q1: serde_json::Value = serde_json::from_str(quarantine_lines[0]).must();
        assert!(q1["errors"].as_array().must().iter().any(|e| {
            e.as_str()
                .must()
                .contains("Required field is missing or null: LastName")
        }));

        let q2: serde_json::Value = serde_json::from_str(quarantine_lines[1]).must();
        assert!(q2["parse_error"].as_str().must().contains("EOF"));

        // Cleanup
        let _ = std::fs::remove_file(&input_path);
        let _ = std::fs::remove_file(&clean_path);
        let _ = std::fs::remove_file(&quarantine_path);
    }
}
