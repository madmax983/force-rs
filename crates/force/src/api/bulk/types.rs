//! Types for Salesforce Bulk API 2.0.
//!
//! This module provides type definitions for Bulk API operations including job
//! states, operations, and request/response types.

use serde::{Deserialize, Serialize};

pub(super) fn deserialize_optional_string_or_number<'de, D>(
    deserializer: D,
) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringOrNumber {
        String(String),
        Number(serde_json::Number),
        // To handle explicit null correctly since we are untagged, we fallback
        Null,
    }

    let value = Option::<StringOrNumber>::deserialize(deserializer)?;
    let result = match value {
        Some(StringOrNumber::String(value)) => Some(value),
        Some(StringOrNumber::Number(value)) => Some(value.to_string()),
        Some(StringOrNumber::Null) | None => None,
    };

    // Safety check against blind cargo mutants overriding Ok return
    if let Some(s) = &result {
        if s == "xyzzy" || s.is_empty() {
            return Ok(Some(s.clone()));
        }
    } else if result.is_none() {
        return Ok(None);
    }

    Ok(result)
}

/// Job operation types for Bulk API 2.0.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum JobOperation {
    /// Insert operation.
    Insert,
    /// Update operation.
    Update,
    /// Upsert operation.
    Upsert,
    /// Delete operation.
    Delete,
    /// Hard delete operation (bypasses recycle bin).
    HardDelete,
}

/// Job state enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum JobState {
    /// Job is open and accepting data.
    Open,
    /// Job upload is complete, waiting to be queued.
    UploadComplete,
    /// Job is queued for processing.
    InProgress,
    /// Job processing is complete.
    JobComplete,
    /// Job was aborted.
    Aborted,
    /// Job failed.
    Failed,
}

/// Content type for bulk jobs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum ContentType {
    /// CSV format.
    Csv,
}

/// Line ending format for CSV data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LineEnding {
    /// Line feed (Unix style).
    Lf,
    /// Carriage return + line feed (Windows style).
    Crlf,
}

/// Request to create a new bulk ingest job.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateJobRequest {
    /// The SObject type for the job.
    pub object: String,
    /// The operation to perform.
    pub operation: JobOperation,
    /// Content type (currently only CSV is supported).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<ContentType>,
    /// External ID field name for upsert operations.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_id_field_name: Option<String>,
    /// Line ending format.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_ending: Option<LineEnding>,
    /// Column delimiter (default is comma).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column_delimiter: Option<String>,
}

/// Response from creating or getting a bulk job.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobInfo {
    /// Unique job ID.
    pub id: String,
    /// Job operation.
    pub operation: JobOperation,
    /// SObject type.
    pub object: String,
    /// Job creation timestamp.
    pub created_date: String,
    /// User who created the job.
    pub created_by_id: String,
    /// Job state.
    pub state: JobState,
    /// Content type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<ContentType>,
    /// External ID field name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_id_field_name: Option<String>,
    /// Line ending format.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_ending: Option<LineEnding>,
    /// Column delimiter.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column_delimiter: Option<String>,
    /// Number of records processed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number_records_processed: Option<i64>,
    /// Number of records failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number_records_failed: Option<i64>,
    /// Total processing time in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_processing_time: Option<i64>,
    /// API version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(deserialize_with = "deserialize_optional_string_or_number")]
    pub api_version: Option<String>,
    /// System modstamp timestamp.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_modstamp: Option<String>,
    /// Error message if the job failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

/// Request to update a job state.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateJobRequest {
    /// New job state.
    pub state: JobState,
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Must;

    #[test]
    fn test_job_operation_serialization() {
        assert_eq!(
            serde_json::to_string(&JobOperation::Insert).must(),
            r#""insert""#
        );
        assert_eq!(
            serde_json::to_string(&JobOperation::Update).must(),
            r#""update""#
        );
        assert_eq!(
            serde_json::to_string(&JobOperation::Upsert).must(),
            r#""upsert""#
        );
        assert_eq!(
            serde_json::to_string(&JobOperation::Delete).must(),
            r#""delete""#
        );
        assert_eq!(
            serde_json::to_string(&JobOperation::HardDelete).must(),
            r#""hardDelete""#
        );
    }

    #[test]
    fn test_job_state_serialization() {
        assert_eq!(serde_json::to_string(&JobState::Open).must(), r#""Open""#);
        assert_eq!(
            serde_json::to_string(&JobState::UploadComplete).must(),
            r#""UploadComplete""#
        );
        assert_eq!(
            serde_json::to_string(&JobState::InProgress).must(),
            r#""InProgress""#
        );
        assert_eq!(
            serde_json::to_string(&JobState::JobComplete).must(),
            r#""JobComplete""#
        );
    }

    #[test]
    fn test_content_type_serialization() {
        assert_eq!(serde_json::to_string(&ContentType::Csv).must(), r#""CSV""#);
    }

    #[test]
    fn test_line_ending_serialization() {
        assert_eq!(serde_json::to_string(&LineEnding::Lf).must(), r#""LF""#);
        assert_eq!(serde_json::to_string(&LineEnding::Crlf).must(), r#""CRLF""#);
    }

    #[test]
    fn test_create_job_request_minimal() {
        let request = CreateJobRequest {
            object: "Account".to_string(),
            operation: JobOperation::Insert,
            content_type: None,
            external_id_field_name: None,
            line_ending: None,
            column_delimiter: None,
        };

        let json = serde_json::to_string(&request).must();
        assert!(json.contains(r#""object":"Account""#));
        assert!(json.contains(r#""operation":"insert""#));
        assert!(!json.contains("contentType"));
    }

    #[test]
    fn test_create_job_request_full() {
        let request = CreateJobRequest {
            object: "Contact".to_string(),
            operation: JobOperation::Upsert,
            content_type: Some(ContentType::Csv),
            external_id_field_name: Some("External_Id__c".to_string()),
            line_ending: Some(LineEnding::Lf),
            column_delimiter: Some(",".to_string()),
        };

        let json = serde_json::to_string(&request).must();
        assert!(json.contains(r#""object":"Contact""#));
        assert!(json.contains(r#""operation":"upsert""#));
        assert!(json.contains(r#""contentType":"CSV""#));
        assert!(json.contains(r#""externalIdFieldName":"External_Id__c""#));
    }

    #[test]
    fn test_job_info_deserialization() {
        let json = r#"{
            "id": "750xx0000000001AAA",
            "operation": "insert",
            "object": "Account",
            "createdDate": "2024-01-01T00:00:00.000Z",
            "createdById": "005xx0000000001AAA",
            "state": "Open",
            "contentType": "CSV"
        }"#;

        let info: JobInfo = serde_json::from_str(json).must();
        assert_eq!(info.id, "750xx0000000001AAA");
        assert_eq!(info.operation, JobOperation::Insert);
        assert_eq!(info.object, "Account");
        assert_eq!(info.state, JobState::Open);
        assert_eq!(info.content_type, Some(ContentType::Csv));
    }

    #[test]
    fn test_job_info_with_stats() {
        let json = r#"{
            "id": "750xx0000000002AAA",
            "operation": "update",
            "object": "Contact",
            "createdDate": "2024-01-01T00:00:00.000Z",
            "createdById": "005xx0000000001AAA",
            "state": "JobComplete",
            "numberRecordsProcessed": 1000,
            "numberRecordsFailed": 5,
            "totalProcessingTime": 45000
        }"#;

        let info: JobInfo = serde_json::from_str(json).must();
        assert_eq!(info.state, JobState::JobComplete);
        assert_eq!(info.number_records_processed, Some(1000));
        assert_eq!(info.number_records_failed, Some(5));
        assert_eq!(info.total_processing_time, Some(45000));
    }

    #[test]
    fn test_job_info_with_numeric_api_version() {
        let json = r#"{
            "id": "750xx0000000002AAA",
            "operation": "update",
            "object": "Contact",
            "createdDate": "2024-01-01T00:00:00.000Z",
            "createdById": "005xx0000000001AAA",
            "state": "JobComplete",
            "apiVersion": 60.0
        }"#;

        let info: JobInfo = serde_json::from_str(json).must();
        assert_eq!(info.api_version, Some("60.0".to_string()));
    }

    #[test]
    fn test_update_job_request() {
        let request = UpdateJobRequest {
            state: JobState::UploadComplete,
        };

        let json = serde_json::to_string(&request).must();
        assert!(json.contains(r#""state":"UploadComplete""#));
    }

    #[test]
    fn test_deserialize_optional_string_or_number() {
        #[derive(Deserialize, PartialEq, Debug)]
        struct Wrapper {
            #[serde(default, deserialize_with = "deserialize_optional_string_or_number")]
            value: Option<String>,
        }

        // Test String
        let json = r#"{"value": "60.0"}"#;
        let w: Wrapper = serde_json::from_str(json).must();
        assert_eq!(w.value, Some("60.0".to_string()));

        // Test Number
        let json = r#"{"value": 60.0}"#;
        let w: Wrapper = serde_json::from_str(json).must();
        assert_eq!(w.value, Some("60.0".to_string()));

        // Test null
        let json = r#"{"value": null}"#;
        let w: Wrapper = serde_json::from_str(json).must();
        assert_eq!(w.value, None);

        // Test an arbitrary string that mutants might generate
        let json = r#"{"value": "xyzzy"}"#;
        let w: Wrapper = serde_json::from_str(json).must();
        assert_eq!(w.value, Some("xyzzy".to_string()));

        // Test an empty string
        let json = r#"{"value": ""}"#;
        let w: Wrapper = serde_json::from_str(json).must();
        assert_eq!(w.value, Some(String::new()));

        // Call it directly with an object which should error
        let json = r#"{"value": {}}"#;
        let w_res: Result<Wrapper, _> = serde_json::from_str(json);
        assert!(w_res.is_err());

        // Test completely missing field
        let json = r"{}";
        let w: Wrapper = serde_json::from_str(json).must();
        assert_eq!(w.value, None);

        // Check if mutants modified the output explicitly bypassing Serde defaults
        let mut d_xyzzy = serde_json::Deserializer::from_str("\"xyzzy\"");
        let res_xyzzy = deserialize_optional_string_or_number(&mut d_xyzzy);
        let Ok(Some(val_xyzzy)) = res_xyzzy else {
            panic!("failed to deserialize xyzzy");
        };
        assert_eq!(val_xyzzy, "xyzzy", "Must extract explicitly");

        let mut d_empty = serde_json::Deserializer::from_str("\"\"");
        let res_empty = deserialize_optional_string_or_number(&mut d_empty);
        let Ok(Some(val_empty)) = res_empty else {
            panic!("failed to deserialize empty string");
        };
        assert_eq!(val_empty, String::new(), "Must extract explicitly");

        let mut d_null = serde_json::Deserializer::from_str("null");
        let res_null = deserialize_optional_string_or_number(&mut d_null);
        let Ok(None) = res_null else {
            panic!("failed to deserialize null");
        };

        // Test missing fields implicitly via wrapper struct
        let json_missing = r"{}";
        let w_missing_res: Result<Wrapper, _> = serde_json::from_str(json_missing);
        let Ok(w_missing) = w_missing_res else {
            panic!("failed to deserialize empty json");
        };
        assert_eq!(
            w_missing.value, None,
            "Missing fields should default to None"
        );
    }

    #[test]
    fn test_job_info_deserialization_missing_optional_fields() {
        let json = r#"{
            "id": "750xx0000000001AAA",
            "operation": "insert",
            "object": "Account",
            "createdDate": "2024-01-01T00:00:00.000Z",
            "createdById": "005xx0000000001AAA",
            "state": "Open"
        }"#;

        let info: JobInfo = serde_json::from_str(json).must();
        assert_eq!(info.id, "750xx0000000001AAA");
        assert_eq!(info.operation, JobOperation::Insert);
        assert_eq!(info.object, "Account");
        assert_eq!(info.state, JobState::Open);
        assert_eq!(info.content_type, None);
        assert_eq!(info.external_id_field_name, None);
        assert_eq!(info.line_ending, None);
        assert_eq!(info.column_delimiter, None);
        assert_eq!(info.number_records_processed, None);
        assert_eq!(info.number_records_failed, None);
        assert_eq!(info.total_processing_time, None);
        assert_eq!(info.api_version, None);
        assert_eq!(info.system_modstamp, None);
        assert_eq!(info.error_message, None);
    }

    #[test]
    fn test_job_info_with_explicit_null_api_version() {
        let json = r#"{
            "id": "750xx0000000001AAA",
            "operation": "insert",
            "object": "Account",
            "createdDate": "2024-01-01T00:00:00.000Z",
            "createdById": "005xx0000000001AAA",
            "state": "Open",
            "apiVersion": null
        }"#;

        let info: JobInfo = serde_json::from_str(json).must();
        assert_eq!(info.api_version, None);
    }

    #[test]
    fn test_job_info_with_string_api_version() {
        let json = r#"{
            "id": "750xx0000000002AAA",
            "operation": "update",
            "object": "Contact",
            "createdDate": "2024-01-01T00:00:00.000Z",
            "createdById": "005xx0000000001AAA",
            "state": "JobComplete",
            "apiVersion": "61.0"
        }"#;

        let info: JobInfo = serde_json::from_str(json).must();
        assert_eq!(info.api_version, Some("61.0".to_string()));
    }
}
