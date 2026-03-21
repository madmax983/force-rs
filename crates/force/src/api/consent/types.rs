//! Consent & Portability API data types.
//!
//! Types for consent status checks and GDPR/CCPA data portability requests.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── Consent Types ────────────────────────────────────────────────────

/// Whether a data subject has consented to an action.
///
/// Unknown values from the API are mapped to [`Unknown`](Self::Unknown)
/// for compliance safety — treat unknown consent as denied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConsentValue {
    /// Consent granted — safe to proceed with the action.
    Yes,
    /// Consent denied — do NOT proceed.
    No,
    /// Consent status unknown — treat as denied for compliance safety.
    Unknown,
}

impl<'de> Deserialize<'de> for ConsentValue {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "Yes" => Ok(Self::Yes),
            "No" => Ok(Self::No),
            _ => Ok(Self::Unknown),
        }
    }
}

impl Serialize for ConsentValue {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Yes => serializer.serialize_str("Yes"),
            Self::No => serializer.serialize_str("No"),
            Self::Unknown => serializer.serialize_str("Unknown"),
        }
    }
}

impl std::fmt::Display for ConsentValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Yes => write!(f, "Yes"),
            Self::No => write!(f, "No"),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

/// The result status of a consent lookup for a single record.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub enum ConsentResult {
    /// Lookup succeeded.
    Success,
    /// Record not found.
    #[serde(rename = "NotFound")]
    NotFound,
    /// An error occurred during lookup.
    Error,
}

/// Consent check result for a single record.
///
/// Contains the lookup status and a map of action → consent value.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ConsentRecord {
    /// Whether the consent lookup succeeded.
    pub result: ConsentResult,

    /// Consent status per action (e.g., `"email"` → `Yes`, `"track"` → `No`).
    #[serde(default)]
    pub proceed: HashMap<String, ConsentValue>,

    /// Catch-all for additional fields.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Top-level response from the consent action/multiaction endpoints.
///
/// Keyed by Salesforce record ID.
pub type ConsentResponse = HashMap<String, ConsentRecord>;

// ── Portability Types ────────────────────────────────────────────────

/// Request to compile data for a GDPR/CCPA portability export.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortabilityRequest {
    /// Object type to export (e.g., `"Contact"`, `"Lead"`).
    #[serde(rename = "objectType")]
    pub object_type: String,

    /// Record IDs to include in the export.
    #[serde(rename = "recordIds")]
    pub record_ids: Vec<String>,

    /// Catch-all for additional fields.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

impl PortabilityRequest {
    /// Creates a new portability request for the given object type and record IDs.
    #[must_use]
    pub fn new(object_type: impl Into<String>, record_ids: Vec<String>) -> Self {
        Self {
            object_type: object_type.into(),
            record_ids,
            extra: HashMap::new(),
        }
    }
}

/// Status of a portability compilation request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortabilityStatus {
    /// Compilation is still in progress.
    Pending,
    /// Compilation complete — download URL is available.
    Complete,
    /// Compilation failed.
    Failed,
}

impl<'de> Deserialize<'de> for PortabilityStatus {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "Complete" => Ok(Self::Complete),
            "Failed" => Ok(Self::Failed),
            _ => Ok(Self::Pending),
        }
    }
}

impl Serialize for PortabilityStatus {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Pending => serializer.serialize_str("Pending"),
            Self::Complete => serializer.serialize_str("Complete"),
            Self::Failed => serializer.serialize_str("Failed"),
        }
    }
}

/// Response from a portability request (POST create or GET status check).
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct PortabilityResponse {
    /// Unique ID for this portability request.
    #[serde(rename = "requestId")]
    pub request_id: String,

    /// Current compilation status.
    pub status: PortabilityStatus,

    /// URL to download compiled data (present when status is `Complete`).
    #[serde(
        rename = "downloadUrl",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub download_url: Option<String>,

    /// Catch-all for additional fields.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Must;

    // ── ConsentValue tests ───────────────────────────────────────────

    #[test]
    fn test_consent_value_deserialize_yes() {
        let v: ConsentValue = serde_json::from_str(r#""Yes""#).must();
        assert_eq!(v, ConsentValue::Yes);
    }

    #[test]
    fn test_consent_value_deserialize_no() {
        let v: ConsentValue = serde_json::from_str(r#""No""#).must();
        assert_eq!(v, ConsentValue::No);
    }

    #[test]
    fn test_consent_value_deserialize_unknown_string() {
        let v: ConsentValue = serde_json::from_str(r#""Maybe""#).must();
        assert_eq!(v, ConsentValue::Unknown);
    }

    #[test]
    fn test_consent_value_deserialize_empty_string() {
        let v: ConsentValue = serde_json::from_str(r#""""#).must();
        assert_eq!(v, ConsentValue::Unknown);
    }

    #[test]
    fn test_consent_value_serialize_roundtrip() {
        let yes_json = serde_json::to_string(&ConsentValue::Yes).must();
        assert_eq!(yes_json, r#""Yes""#);
        let no_json = serde_json::to_string(&ConsentValue::No).must();
        assert_eq!(no_json, r#""No""#);
        let unknown_json = serde_json::to_string(&ConsentValue::Unknown).must();
        assert_eq!(unknown_json, r#""Unknown""#);
    }

    #[test]
    fn test_consent_value_display() {
        assert_eq!(ConsentValue::Yes.to_string(), "Yes");
        assert_eq!(ConsentValue::No.to_string(), "No");
        assert_eq!(ConsentValue::Unknown.to_string(), "Unknown");
    }

    // ── ConsentResult tests ──────────────────────────────────────────

    #[test]
    fn test_consent_result_deserialize() {
        let success: ConsentResult = serde_json::from_str(r#""Success""#).must();
        assert_eq!(success, ConsentResult::Success);
        let not_found: ConsentResult = serde_json::from_str(r#""NotFound""#).must();
        assert_eq!(not_found, ConsentResult::NotFound);
        let error: ConsentResult = serde_json::from_str(r#""Error""#).must();
        assert_eq!(error, ConsentResult::Error);
    }

    // ── ConsentRecord tests ──────────────────────────────────────────

    #[test]
    fn test_consent_record_deserialize() {
        let json = serde_json::json!({
            "result": "Success",
            "proceed": {
                "email": "Yes",
                "track": "No"
            }
        });

        let record: ConsentRecord = serde_json::from_value(json).must();
        assert_eq!(record.result, ConsentResult::Success);
        assert_eq!(record.proceed["email"], ConsentValue::Yes);
        assert_eq!(record.proceed["track"], ConsentValue::No);
    }

    #[test]
    fn test_consent_record_not_found() {
        let json = serde_json::json!({
            "result": "NotFound",
            "proceed": {}
        });

        let record: ConsentRecord = serde_json::from_value(json).must();
        assert_eq!(record.result, ConsentResult::NotFound);
        assert!(record.proceed.is_empty());
    }

    #[test]
    fn test_consent_record_with_unknown_consent() {
        let json = serde_json::json!({
            "result": "Success",
            "proceed": {
                "email": "Yes",
                "customAction": "SomeWeirdValue"
            }
        });

        let record: ConsentRecord = serde_json::from_value(json).must();
        assert_eq!(record.proceed["email"], ConsentValue::Yes);
        assert_eq!(record.proceed["customAction"], ConsentValue::Unknown);
    }

    #[test]
    fn test_consent_record_captures_extras() {
        let json = serde_json::json!({
            "result": "Success",
            "proceed": {"email": "Yes"},
            "someExtraField": "extraValue"
        });

        let record: ConsentRecord = serde_json::from_value(json).must();
        assert!(record.extra.contains_key("someExtraField"));
    }

    #[test]
    fn test_consent_response_multiple_records() {
        let json = serde_json::json!({
            "001xx000003GYk1": {
                "result": "Success",
                "proceed": {"email": "Yes"}
            },
            "001xx000003GYk2": {
                "result": "Success",
                "proceed": {"email": "No"}
            }
        });

        let response: ConsentResponse = serde_json::from_value(json).must();
        assert_eq!(response.len(), 2);
        assert_eq!(
            response["001xx000003GYk1"].proceed["email"],
            ConsentValue::Yes
        );
        assert_eq!(
            response["001xx000003GYk2"].proceed["email"],
            ConsentValue::No
        );
    }

    // ── PortabilityRequest tests ─────────────────────────────────────

    #[test]
    fn test_portability_request_new() {
        let req = PortabilityRequest::new("Contact", vec!["003xx000003GYk1".to_string()]);
        assert_eq!(req.object_type, "Contact");
        assert_eq!(req.record_ids.len(), 1);
    }

    #[test]
    fn test_portability_request_serializes() {
        let req = PortabilityRequest::new("Contact", vec!["003xx000003GYk1".to_string()]);
        let json = serde_json::to_value(&req).must();
        assert_eq!(json["objectType"], "Contact");
        assert_eq!(json["recordIds"][0], "003xx000003GYk1");
    }

    // ── PortabilityStatus tests ──────────────────────────────────────

    #[test]
    fn test_portability_status_deserialize() {
        let pending: PortabilityStatus = serde_json::from_str(r#""Pending""#).must();
        assert_eq!(pending, PortabilityStatus::Pending);
        let complete: PortabilityStatus = serde_json::from_str(r#""Complete""#).must();
        assert_eq!(complete, PortabilityStatus::Complete);
        let failed: PortabilityStatus = serde_json::from_str(r#""Failed""#).must();
        assert_eq!(failed, PortabilityStatus::Failed);
    }

    #[test]
    fn test_portability_status_unknown_maps_to_pending() {
        let status: PortabilityStatus = serde_json::from_str(r#""InProgress""#).must();
        assert_eq!(status, PortabilityStatus::Pending);
    }

    // ── PortabilityResponse tests ────────────────────────────────────

    #[test]
    fn test_portability_response_pending() {
        let json = serde_json::json!({
            "requestId": "req-001",
            "status": "Pending"
        });

        let resp: PortabilityResponse = serde_json::from_value(json).must();
        assert_eq!(resp.request_id, "req-001");
        assert_eq!(resp.status, PortabilityStatus::Pending);
        assert!(resp.download_url.is_none());
    }

    #[test]
    fn test_portability_response_complete() {
        let json = serde_json::json!({
            "requestId": "req-001",
            "status": "Complete",
            "downloadUrl": "https://instance.salesforce.com/download/portability/req-001"
        });

        let resp: PortabilityResponse = serde_json::from_value(json).must();
        assert_eq!(resp.status, PortabilityStatus::Complete);
        assert_eq!(
            resp.download_url.as_deref(),
            Some("https://instance.salesforce.com/download/portability/req-001")
        );
    }

    #[test]
    fn test_portability_response_failed() {
        let json = serde_json::json!({
            "requestId": "req-001",
            "status": "Failed"
        });

        let resp: PortabilityResponse = serde_json::from_value(json).must();
        assert_eq!(resp.status, PortabilityStatus::Failed);
        assert!(resp.download_url.is_none());
    }

    #[test]
    fn test_portability_response_captures_extras() {
        let json = serde_json::json!({
            "requestId": "req-001",
            "status": "Pending",
            "estimatedCompletionTime": "2026-03-21T15:00:00Z"
        });

        let resp: PortabilityResponse = serde_json::from_value(json).must();
        assert!(resp.extra.contains_key("estimatedCompletionTime"));
    }
}
