//! Types for Salesforce Bulk API 2.0.
//!
//! This module provides type definitions for Bulk API operations including job
//! states, operations, and request/response types.

use serde::{Deserialize, Serialize};

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

/// Job state enumeration using typestate pattern.
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_version: Option<String>,
    /// System modstamp timestamp.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_modstamp: Option<String>,
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

    #[test]
    fn test_job_operation_serialization() {
        assert_eq!(
            serde_json::to_string(&JobOperation::Insert).unwrap(),
            r#""insert""#
        );
        assert_eq!(
            serde_json::to_string(&JobOperation::Update).unwrap(),
            r#""update""#
        );
        assert_eq!(
            serde_json::to_string(&JobOperation::Upsert).unwrap(),
            r#""upsert""#
        );
        assert_eq!(
            serde_json::to_string(&JobOperation::Delete).unwrap(),
            r#""delete""#
        );
        assert_eq!(
            serde_json::to_string(&JobOperation::HardDelete).unwrap(),
            r#""hardDelete""#
        );
    }

    #[test]
    fn test_job_state_serialization() {
        assert_eq!(serde_json::to_string(&JobState::Open).unwrap(), r#""Open""#);
        assert_eq!(
            serde_json::to_string(&JobState::UploadComplete).unwrap(),
            r#""UploadComplete""#
        );
        assert_eq!(
            serde_json::to_string(&JobState::InProgress).unwrap(),
            r#""InProgress""#
        );
        assert_eq!(
            serde_json::to_string(&JobState::JobComplete).unwrap(),
            r#""JobComplete""#
        );
    }

    #[test]
    fn test_content_type_serialization() {
        assert_eq!(
            serde_json::to_string(&ContentType::Csv).unwrap(),
            r#""CSV""#
        );
    }

    #[test]
    fn test_line_ending_serialization() {
        assert_eq!(serde_json::to_string(&LineEnding::Lf).unwrap(), r#""LF""#);
        assert_eq!(
            serde_json::to_string(&LineEnding::Crlf).unwrap(),
            r#""CRLF""#
        );
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

        let json = serde_json::to_string(&request).unwrap();
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

        let json = serde_json::to_string(&request).unwrap();
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

        let info: JobInfo = serde_json::from_str(json).unwrap();
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

        let info: JobInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.state, JobState::JobComplete);
        assert_eq!(info.number_records_processed, Some(1000));
        assert_eq!(info.number_records_failed, Some(5));
        assert_eq!(info.total_processing_time, Some(45000));
    }

    #[test]
    fn test_update_job_request() {
        let request = UpdateJobRequest {
            state: JobState::UploadComplete,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains(r#""state":"UploadComplete""#));
    }
}
