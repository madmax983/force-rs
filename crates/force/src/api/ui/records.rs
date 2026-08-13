//! UI API record endpoints.
//!
//! Provides layout-aware CRUD operations and record-ui aggregation via the
//! Salesforce UI API (`/services/data/vXX.0/ui-api/records/` and related paths).

#![allow(clippy::doc_markdown)]

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

// ─── Response types ──────────────────────────────────────────────────────────

/// Aggregated layout-aware record data for one or more Salesforce records.
///
/// Returned by `record_ui()`. Contains layout metadata, object info, and the
/// records themselves, all keyed by record ID.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordUiRepresentation {
    /// User-specific layout state keyed by layout ID.
    pub layout_user_states: HashMap<String, Value>,
    /// Layout metadata keyed by layout ID.
    pub layouts: HashMap<String, Value>,
    /// Object metadata keyed by API name.
    pub object_infos: HashMap<String, Value>,
    /// Record data keyed by record ID.
    pub records: HashMap<String, RecordRepresentation>,
    /// Any additional fields returned by the API.
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// A single layout-aware Salesforce record representation.
///
/// Returned by `get_record()`, `create_record()`, `update_record()`, and
/// embedded in `RecordUiRepresentation` and `RecordDefaultsRepresentation`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordRepresentation {
    /// The SObject API name (e.g., `"Account"`).
    pub api_name: String,
    /// Record ID (`None` for new-record defaults).
    pub id: Option<String>,
    /// Fields, each wrapped in a `FieldValueRepresentation`.
    pub fields: HashMap<String, crate::api::ui::types::FieldValueRepresentation>,
    /// Child relationship data keyed by relationship name.
    pub child_relationships: HashMap<String, Value>,
    /// Record type ID, if applicable.
    pub record_type_id: Option<String>,
    /// Last modification timestamp from the system.
    pub system_modstamp: Option<String>,
    /// Any additional fields returned by the API.
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// Batch result wrapper containing multiple record fetch results.
///
/// Returned by `get_records_batch()`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchResultRepresentation {
    /// `true` if any individual result contained an error.
    pub has_errors: bool,
    /// Individual result entries (may be records or error objects).
    pub results: Vec<Value>,
    /// Any additional fields returned by the API.
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// Default field values for a new or cloned record.
///
/// Returned by `create_defaults()` and `clone_defaults()`.
///
/// The UI API "Get Record Defaults for Create/Clone" response mirrors the
/// `record-ui` shape: it carries `objectInfos` (a **plural** map of
/// objectApiName → object metadata), not a singular `objectInfo`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordDefaultsRepresentation {
    /// Layout information relevant to the create/clone context.
    #[serde(default)]
    pub layout: Value,
    /// Object metadata keyed by object API name (target plus any referenced objects).
    #[serde(default)]
    pub object_infos: HashMap<String, crate::api::ui::object_info::ObjectInfoRepresentation>,
    /// A record representation pre-populated with default values.
    pub record: RecordRepresentation,
    /// Any additional fields returned by the API.
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

// ─── Input types ─────────────────────────────────────────────────────────────

/// Input body for creating a new record via the UI API.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateRecordInput {
    /// SObject API name (e.g., `"Contact"`).
    pub api_name: String,
    /// Field values to set on the new record.
    pub fields: HashMap<String, Value>,
}

/// Input body for updating an existing record via the UI API.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRecordInput {
    /// Field values to update on the record.
    pub fields: HashMap<String, Value>,
}

// ─── UiHandler<A> implementation ─────────────────────────────────────────────

impl<A: crate::auth::Authenticator> crate::api::ui::UiHandler<A> {
    /// Returns layout-aware record data for one or more records.
    ///
    /// Calls `GET /ui-api/record-ui/{ids}`.
    ///
    /// * `ids` – one or more Salesforce record IDs.
    /// * `layout_types` – optional layout type filter (`Compact`, `Full`, …).
    /// * `modes` – optional mode filter (`Create`, `Edit`, `View`).
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP request fails or any ID is invalid.
    pub async fn record_ui(
        &self,
        ids: &[&str],
        layout_types: Option<&[crate::api::ui::types::LayoutType]>,
        modes: Option<&[crate::api::ui::types::Mode]>,
    ) -> crate::error::Result<RecordUiRepresentation> {
        for id in ids {
            crate::types::validator::validate_identifier(id, "record id")?;
        }

        let ids_str = ids.join(",");
        let path = format!("record-ui/{}", ids_str);

        // ⚡ Bolt: Eliminates intermediate heap allocations by pre-allocating string capacity and joining manually.
        let lt_str = layout_types.map(|lts| {
            if lts.is_empty() {
                return String::new();
            }
            let mut s = String::with_capacity(lts.len() * 10);
            for (i, lt) in lts.iter().enumerate() {
                if i > 0 {
                    s.push(',');
                }
                s.push_str(lt.as_str());
            }
            s
        });

        // ⚡ Bolt: Eliminates intermediate heap allocations by pre-allocating string capacity and joining manually.
        let mode_str = modes.map(|ms| {
            if ms.is_empty() {
                return String::new();
            }
            let mut s = String::with_capacity(ms.len() * 10);
            for (i, m) in ms.iter().enumerate() {
                if i > 0 {
                    s.push(',');
                }
                s.push_str(m.as_str());
            }
            s
        });

        // ⚡ Bolt: Use a stack-allocated array to avoid heap allocation for small parameter list
        let mut params_array = [("", ""); 2];
        let mut params_len = 0;

        if let Some(lts) = &lt_str {
            params_array[params_len] = ("layoutTypes", lts);
            params_len += 1;
        }

        if let Some(ms) = &mode_str {
            params_array[params_len] = ("modes", ms);
            params_len += 1;
        }

        let query = if params_len == 0 {
            None
        } else {
            Some(&params_array[..params_len])
        };

        self.get(&path, query, "Failed to fetch record UI").await
    }

    /// Returns a single layout-aware record.
    ///
    /// Calls `GET /ui-api/records/{id}`.
    ///
    /// * `id` – Salesforce record ID.
    /// * `fields` – optional list of field API names to include.
    ///
    /// # Errors
    ///
    /// Returns an error if the record is not found or the request fails.
    pub async fn get_record(
        &self,
        id: &str,
        fields: Option<&[&str]>,
    ) -> crate::error::Result<RecordRepresentation> {
        crate::types::validator::validate_identifier(id, "record id")?;

        let path = format!("records/{id}");

        let mut fields_str = String::new();

        // ⚡ Bolt: Use a stack-allocated array to avoid heap allocation for small parameter list
        let mut params_array = [("", ""); 1];
        let mut params_len = 0;

        if let Some(fs) = fields {
            // ⚡ Bolt: Construct string directly to avoid intermediate `.join(",")` allocation
            fields_str.reserve(fs.len() * 20);
            for (i, f) in fs.iter().enumerate() {
                if i > 0 {
                    fields_str.push(',');
                }
                fields_str.push_str(f);
            }
            params_array[params_len] = ("fields", &fields_str);
            params_len += 1;
        }

        let query = if params_len == 0 {
            None
        } else {
            Some(&params_array[..params_len])
        };

        self.get(&path, query, "Failed to fetch record").await
    }

    /// Returns layout-aware data for multiple records in a single request.
    ///
    /// Calls `GET /ui-api/records/batch/{ids}`.
    ///
    /// * `ids` – one or more Salesforce record IDs.
    /// * `fields` – optional list of field API names to include.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails.
    pub async fn get_records_batch(
        &self,
        ids: &[&str],
        fields: Option<&[&str]>,
    ) -> crate::error::Result<BatchResultRepresentation> {
        for id in ids {
            crate::types::validator::validate_identifier(id, "record id")?;
        }

        let path = format!("records/batch/{}", ids.join(","));
        let fields_str = fields.map(|fs| fs.join(","));

        // ⚡ Bolt: Use a stack-allocated array to avoid heap allocation for small parameter list
        let mut params_array = [("", ""); 1];
        let mut params_len = 0;

        if let Some(fs) = &fields_str {
            params_array[params_len] = ("fields", fs);
            params_len += 1;
        }

        let query = if params_len == 0 {
            None
        } else {
            Some(&params_array[..params_len])
        };

        self.get(&path, query, "Failed to fetch records batch")
            .await
    }

    /// Creates a new Salesforce record via the UI API.
    ///
    /// Calls `POST /ui-api/records`.
    ///
    /// # Errors
    ///
    /// Returns an error if required fields are missing or the request fails.
    pub async fn create_record(
        &self,
        input: &CreateRecordInput,
    ) -> crate::error::Result<RecordRepresentation> {
        self.post("records", input, "Failed to create record").await
    }

    /// Updates an existing Salesforce record via the UI API.
    ///
    /// Calls `PATCH /ui-api/records/{id}`.
    ///
    /// # Errors
    ///
    /// Returns an error if the record is not found or the update is invalid.
    pub async fn update_record(
        &self,
        id: &str,
        input: &UpdateRecordInput,
    ) -> crate::error::Result<RecordRepresentation> {
        crate::types::validator::validate_identifier(id, "record id")?;

        let path = format!("records/{id}");
        self.patch(&path, input, "Failed to update record").await
    }

    /// Deletes a Salesforce record via the UI API.
    ///
    /// Calls `DELETE /ui-api/records/{id}`. Returns `()` on success (204).
    ///
    /// # Errors
    ///
    /// Returns an error if the record is not found or deletion is not allowed.
    pub async fn delete_record(&self, id: &str) -> crate::error::Result<()> {
        crate::types::validator::validate_identifier(id, "record id")?;

        let path = format!("records/{id}");
        self.delete_empty(&path, "Failed to delete record").await
    }

    /// Returns default field values for creating a new record.
    ///
    /// Calls `GET /ui-api/record-defaults/create/{object}`.
    ///
    /// # Errors
    ///
    /// Returns an error if the object type is unknown or the request fails.
    pub async fn create_defaults(
        &self,
        object: &str,
    ) -> crate::error::Result<RecordDefaultsRepresentation> {
        crate::types::validator::validate_sobject_name(object)?;

        let path = format!("record-defaults/create/{object}");
        self.get(&path, None, "Failed to fetch create defaults")
            .await
    }

    /// Returns default field values pre-populated from an existing record (clone).
    ///
    /// Calls `GET /ui-api/record-defaults/clone/{id}`.
    ///
    /// # Errors
    ///
    /// Returns an error if the source record is not found or the request fails.
    pub async fn clone_defaults(
        &self,
        id: &str,
    ) -> crate::error::Result<RecordDefaultsRepresentation> {
        crate::types::validator::validate_identifier(id, "record id")?;

        let path = format!("record-defaults/clone/{id}");
        self.get(&path, None, "Failed to fetch clone defaults")
            .await
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use crate::test_utils::must::Must;

    use super::*;
    use crate::api::ui::types::{LayoutType, Mode};
    use crate::client::builder;
    use crate::test_utils::mock_auth::MockAuthenticator;
    use serde_json::json;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const VALID_ID: &str = "001000000000001AAA";
    const VALID_ID2: &str = "001000000000002AAA";

    async fn make_client(server: &MockServer) -> crate::client::ForceClient<MockAuthenticator> {
        let auth = MockAuthenticator::new("test_token", &server.uri());
        builder().authenticate(auth).build().await.must()
    }

    fn minimal_record_json(id: &str) -> serde_json::Value {
        json!({
            "apiName": "Account",
            "id": id,
            "fields": {
                "Name": {
                    "displayValue": "Acme",
                    "value": "Acme"
                }
            },
            "childRelationships": {},
            "recordTypeId": null,
            "systemModstamp": null
        })
    }

    /// Minimal but real-shaped `ObjectInfoRepresentation` for use as an
    /// `objectInfos` map value in record-defaults responses.
    fn minimal_object_info_json(api_name: &str) -> serde_json::Value {
        json!({
            "apiName": api_name,
            "label": api_name,
            "labelPlural": format!("{api_name}s"),
            "keyPrefix": "001",
            "fields": {
                "Name": {
                    "apiName": "Name",
                    "label": "Name",
                    "dataType": "String",
                    "required": true,
                    "updateable": true,
                    "createable": true,
                    "referenceToInfos": []
                }
            }
        })
    }

    // ── record_ui ────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_record_ui_success_single_id() {
        let server = MockServer::start().await;
        let client = make_client(&server).await;

        let response_body = json!({
            "layoutUserStates": {},
            "layouts": {},
            "objectInfos": {},
            "records": {
                VALID_ID: minimal_record_json(VALID_ID)
            }
        });

        Mock::given(method("GET"))
            .and(path(format!(
                "/services/data/v67.0/ui-api/record-ui/{VALID_ID}"
            )))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
            .expect(1)
            .mount(&server)
            .await;

        let result = client.ui().record_ui(&[VALID_ID], None, None).await.must();

        assert!(result.records.contains_key(VALID_ID));
    }

    #[tokio::test]
    async fn test_record_ui_with_layout_types_and_modes() {
        let server = MockServer::start().await;
        let client = make_client(&server).await;

        let response_body = json!({
            "layoutUserStates": {},
            "layouts": {},
            "objectInfos": {},
            "records": {
                VALID_ID: minimal_record_json(VALID_ID)
            }
        });

        Mock::given(method("GET"))
            .and(path(format!(
                "/services/data/v67.0/ui-api/record-ui/{VALID_ID}"
            )))
            .and(query_param("layoutTypes", "Full"))
            .and(query_param("modes", "View"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
            .expect(1)
            .mount(&server)
            .await;

        let result = client
            .ui()
            .record_ui(&[VALID_ID], Some(&[LayoutType::Full]), Some(&[Mode::View]))
            .await
            .must();

        assert!(result.records.contains_key(VALID_ID));
    }

    #[tokio::test]
    async fn test_record_ui_multiple_ids() {
        let server = MockServer::start().await;
        let client = make_client(&server).await;

        let joined = format!("{VALID_ID},{VALID_ID2}");
        let response_body = json!({
            "layoutUserStates": {},
            "layouts": {},
            "objectInfos": {},
            "records": {
                VALID_ID:  minimal_record_json(VALID_ID),
                VALID_ID2: minimal_record_json(VALID_ID2)
            }
        });

        Mock::given(method("GET"))
            .and(path(format!(
                "/services/data/v67.0/ui-api/record-ui/{joined}"
            )))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
            .expect(1)
            .mount(&server)
            .await;

        let result = client
            .ui()
            .record_ui(&[VALID_ID, VALID_ID2], None, None)
            .await
            .must();

        assert_eq!(result.records.len(), 2);
    }

    #[tokio::test]
    async fn test_record_ui_not_found() {
        let server = MockServer::start().await;
        let client = make_client(&server).await;

        Mock::given(method("GET"))
            .and(path(format!(
                "/services/data/v67.0/ui-api/record-ui/{VALID_ID}"
            )))
            .respond_with(ResponseTemplate::new(404).set_body_json(json!([{
                "errorCode": "NOT_FOUND",
                "message": "Record not found"
            }])))
            .expect(1)
            .mount(&server)
            .await;

        let result = client.ui().record_ui(&[VALID_ID], None, None).await;
        let Err(err) = result else {
            panic!("Expected an error");
        };
        assert!(
            matches!(
                err,
                crate::error::ForceError::Api(_) | crate::error::ForceError::Http(_)
            ),
            "Expected Api or Http error, got: {err}"
        );
    }

    // ── get_record ───────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_get_record_success_no_fields() {
        let server = MockServer::start().await;
        let client = make_client(&server).await;

        Mock::given(method("GET"))
            .and(path(format!(
                "/services/data/v67.0/ui-api/records/{VALID_ID}"
            )))
            .respond_with(ResponseTemplate::new(200).set_body_json(minimal_record_json(VALID_ID)))
            .expect(1)
            .mount(&server)
            .await;

        let record = client.ui().get_record(VALID_ID, None).await.must();
        assert_eq!(record.api_name, "Account");
        assert_eq!(record.id.as_deref(), Some(VALID_ID));
    }

    #[tokio::test]
    async fn test_get_record_with_fields_param() {
        let server = MockServer::start().await;
        let client = make_client(&server).await;

        Mock::given(method("GET"))
            .and(path(format!(
                "/services/data/v67.0/ui-api/records/{VALID_ID}"
            )))
            .and(query_param("fields", "Account.Name,Account.Phone"))
            .respond_with(ResponseTemplate::new(200).set_body_json(minimal_record_json(VALID_ID)))
            .expect(1)
            .mount(&server)
            .await;

        let record = client
            .ui()
            .get_record(VALID_ID, Some(&["Account.Name", "Account.Phone"]))
            .await
            .must();
        assert_eq!(record.api_name, "Account");
    }

    #[tokio::test]
    async fn test_get_record_not_found() {
        let server = MockServer::start().await;
        let client = make_client(&server).await;

        Mock::given(method("GET"))
            .and(path(format!(
                "/services/data/v67.0/ui-api/records/{VALID_ID}"
            )))
            .respond_with(ResponseTemplate::new(404).set_body_json(json!([{
                "errorCode": "NOT_FOUND",
                "message": "Record not found"
            }])))
            .expect(1)
            .mount(&server)
            .await;

        let result = client.ui().get_record(VALID_ID, None).await;
        let Err(err) = result else {
            panic!("Expected an error");
        };
        assert!(
            matches!(
                err,
                crate::error::ForceError::Api(_) | crate::error::ForceError::Http(_)
            ),
            "Expected Api or Http error, got: {err}"
        );
    }

    // ── get_records_batch ────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_get_records_batch_success() {
        let server = MockServer::start().await;
        let client = make_client(&server).await;

        let joined = format!("{VALID_ID},{VALID_ID2}");
        let response_body = json!({
            "hasErrors": false,
            "results": [
                minimal_record_json(VALID_ID),
                minimal_record_json(VALID_ID2)
            ]
        });

        Mock::given(method("GET"))
            .and(path(format!(
                "/services/data/v67.0/ui-api/records/batch/{joined}"
            )))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
            .expect(1)
            .mount(&server)
            .await;

        let result = client
            .ui()
            .get_records_batch(&[VALID_ID, VALID_ID2], None)
            .await
            .must();

        assert!(!result.has_errors);
        assert_eq!(result.results.len(), 2);
    }

    #[tokio::test]
    async fn test_get_records_batch_with_fields() {
        let server = MockServer::start().await;
        let client = make_client(&server).await;

        let response_body = json!({
            "hasErrors": false,
            "results": [minimal_record_json(VALID_ID)]
        });

        Mock::given(method("GET"))
            .and(path(format!(
                "/services/data/v67.0/ui-api/records/batch/{VALID_ID}"
            )))
            .and(query_param("fields", "Account.Name"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
            .expect(1)
            .mount(&server)
            .await;

        let result = client
            .ui()
            .get_records_batch(&[VALID_ID], Some(&["Account.Name"]))
            .await
            .must();

        assert!(!result.has_errors);
    }

    // ── create_record ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_create_record_success() {
        let server = MockServer::start().await;
        let client = make_client(&server).await;

        Mock::given(method("POST"))
            .and(path("/services/data/v67.0/ui-api/records"))
            .respond_with(ResponseTemplate::new(200).set_body_json(minimal_record_json(VALID_ID)))
            .expect(1)
            .mount(&server)
            .await;

        let input = CreateRecordInput {
            api_name: "Account".to_string(),
            fields: {
                let mut m = HashMap::new();
                m.insert("Name".to_string(), json!("Acme Corp"));
                m
            },
        };

        let record = client.ui().create_record(&input).await.must();
        assert_eq!(record.api_name, "Account");
    }

    #[tokio::test]
    async fn test_create_record_bad_request() {
        let server = MockServer::start().await;
        let client = make_client(&server).await;

        Mock::given(method("POST"))
            .and(path("/services/data/v67.0/ui-api/records"))
            .respond_with(ResponseTemplate::new(400).set_body_json(json!([{
                "errorCode": "REQUIRED_FIELD_MISSING",
                "message": "Required fields are missing: [Name]"
            }])))
            .expect(1)
            .mount(&server)
            .await;

        let input = CreateRecordInput {
            api_name: "Account".to_string(),
            fields: HashMap::new(),
        };

        let result = client.ui().create_record(&input).await;
        let Err(err) = result else {
            panic!("Expected an error");
        };
        assert!(
            matches!(
                err,
                crate::error::ForceError::Api(_) | crate::error::ForceError::Http(_)
            ),
            "Expected Api or Http error, got: {err}"
        );
    }

    // ── update_record ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_update_record_success() {
        let server = MockServer::start().await;
        let client = make_client(&server).await;

        Mock::given(method("PATCH"))
            .and(path(format!(
                "/services/data/v67.0/ui-api/records/{VALID_ID}"
            )))
            .respond_with(ResponseTemplate::new(200).set_body_json(minimal_record_json(VALID_ID)))
            .expect(1)
            .mount(&server)
            .await;

        let input = UpdateRecordInput {
            fields: {
                let mut m = HashMap::new();
                m.insert("Phone".to_string(), json!("+1-555-0100"));
                m
            },
        };

        let record = client.ui().update_record(VALID_ID, &input).await.must();
        assert_eq!(record.api_name, "Account");
    }

    #[tokio::test]
    async fn test_update_record_not_found() {
        let server = MockServer::start().await;
        let client = make_client(&server).await;

        Mock::given(method("PATCH"))
            .and(path(format!(
                "/services/data/v67.0/ui-api/records/{VALID_ID}"
            )))
            .respond_with(ResponseTemplate::new(404).set_body_json(json!([{
                "errorCode": "NOT_FOUND",
                "message": "Record not found"
            }])))
            .expect(1)
            .mount(&server)
            .await;

        let input = UpdateRecordInput {
            fields: HashMap::new(),
        };

        let result = client.ui().update_record(VALID_ID, &input).await;
        let Err(err) = result else {
            panic!("Expected an error");
        };
        assert!(
            matches!(
                err,
                crate::error::ForceError::Api(_) | crate::error::ForceError::Http(_)
            ),
            "Expected Api or Http error, got: {err}"
        );
    }

    // ── delete_record ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_delete_record_success() {
        let server = MockServer::start().await;
        let client = make_client(&server).await;

        Mock::given(method("DELETE"))
            .and(path(format!(
                "/services/data/v67.0/ui-api/records/{VALID_ID}"
            )))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;

        client.ui().delete_record(VALID_ID).await.must();
    }

    #[tokio::test]
    async fn test_delete_record_not_found() {
        let server = MockServer::start().await;
        let client = make_client(&server).await;

        Mock::given(method("DELETE"))
            .and(path(format!(
                "/services/data/v67.0/ui-api/records/{VALID_ID}"
            )))
            .respond_with(ResponseTemplate::new(404).set_body_json(json!([{
                "errorCode": "NOT_FOUND",
                "message": "Record not found"
            }])))
            .expect(1)
            .mount(&server)
            .await;

        let result = client.ui().delete_record(VALID_ID).await;
        let Err(err) = result else {
            panic!("Expected an error");
        };
        assert!(
            matches!(
                err,
                crate::error::ForceError::Api(_) | crate::error::ForceError::Http(_)
            ),
            "Expected Api or Http error, got: {err}"
        );
    }

    // ── create_defaults ──────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_create_defaults_success() {
        let server = MockServer::start().await;
        let client = make_client(&server).await;

        // Build a record with no ID for defaults
        let defaults_record = json!({
            "apiName": "Account",
            "id": null,
            "fields": {},
            "childRelationships": {},
            "recordTypeId": null,
            "systemModstamp": null
        });

        let response = json!({
            "layout": {},
            "objectInfos": {
                "Account": minimal_object_info_json("Account")
            },
            "record": defaults_record
        });

        Mock::given(method("GET"))
            .and(path(
                "/services/data/v67.0/ui-api/record-defaults/create/Account",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response))
            .expect(1)
            .mount(&server)
            .await;

        let result = client.ui().create_defaults("Account").await.must();
        assert_eq!(result.record.api_name, "Account");
        // The plural `objectInfos` map is populated and keyed by API name.
        assert!(result.object_infos.contains_key("Account"));
        assert_eq!(result.object_infos["Account"].api_name, "Account");
    }

    #[tokio::test]
    async fn test_create_defaults_unknown_object() {
        let server = MockServer::start().await;
        let client = make_client(&server).await;

        Mock::given(method("GET"))
            .and(path(
                "/services/data/v67.0/ui-api/record-defaults/create/NoSuchObject",
            ))
            .respond_with(ResponseTemplate::new(404).set_body_json(json!([{
                "errorCode": "NOT_FOUND",
                "message": "The requested resource does not exist"
            }])))
            .expect(1)
            .mount(&server)
            .await;

        let result = client.ui().create_defaults("NoSuchObject").await;
        let Err(err) = result else {
            panic!("Expected an error");
        };
        assert!(
            matches!(
                err,
                crate::error::ForceError::Api(_) | crate::error::ForceError::Http(_)
            ),
            "Expected Api or Http error, got: {err}"
        );
    }

    // ── clone_defaults ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_clone_defaults_success() {
        let server = MockServer::start().await;
        let client = make_client(&server).await;

        let clone_record = json!({
            "apiName": "Account",
            "id": null,
            "fields": {
                "Name": {
                    "displayValue": "Acme (Copy)",
                    "value": "Acme (Copy)"
                }
            },
            "childRelationships": {},
            "recordTypeId": null,
            "systemModstamp": null
        });

        let response = json!({
            "layout": {},
            "objectInfos": {
                "Account": minimal_object_info_json("Account")
            },
            "record": clone_record
        });

        Mock::given(method("GET"))
            .and(path(format!(
                "/services/data/v67.0/ui-api/record-defaults/clone/{VALID_ID}"
            )))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response))
            .expect(1)
            .mount(&server)
            .await;

        let result = client.ui().clone_defaults(VALID_ID).await.must();
        assert_eq!(result.record.api_name, "Account");
        assert!(result.record.id.is_none());
    }

    #[tokio::test]
    async fn test_clone_defaults_not_found() {
        let server = MockServer::start().await;
        let client = make_client(&server).await;

        Mock::given(method("GET"))
            .and(path(format!(
                "/services/data/v67.0/ui-api/record-defaults/clone/{VALID_ID}"
            )))
            .respond_with(ResponseTemplate::new(404).set_body_json(json!([{
                "errorCode": "NOT_FOUND",
                "message": "Record not found"
            }])))
            .expect(1)
            .mount(&server)
            .await;

        let result = client.ui().clone_defaults(VALID_ID).await;
        let Err(err) = result else {
            panic!("Expected an error");
        };
        assert!(
            matches!(
                err,
                crate::error::ForceError::Api(_) | crate::error::ForceError::Http(_)
            ),
            "Expected Api or Http error, got: {err}"
        );
    }

    // ── type deserialization ─────────────────────────────────────────────────

    #[test]
    fn test_record_representation_deserialize() {
        let json_str = r#"{
            "apiName": "Contact",
            "id": "003000000000001AAA",
            "fields": {
                "FirstName": {"displayValue": "Jane", "value": "Jane"}
            },
            "childRelationships": {},
            "recordTypeId": null,
            "systemModstamp": "2024-01-01T00:00:00.000Z"
        }"#;

        let record: RecordRepresentation = serde_json::from_str(json_str).must();
        assert_eq!(record.api_name, "Contact");
        assert_eq!(record.id.as_deref(), Some("003000000000001AAA"));
        assert!(record.fields.contains_key("FirstName"));
        assert_eq!(
            record.system_modstamp.as_deref(),
            Some("2024-01-01T00:00:00.000Z")
        );
    }

    #[test]
    fn test_create_record_input_serialize() {
        let mut fields = HashMap::new();
        fields.insert("Name".to_string(), json!("Test"));

        let input = CreateRecordInput {
            api_name: "Account".to_string(),
            fields,
        };

        let serialized = serde_json::to_value(&input).must();
        assert_eq!(serialized["apiName"], "Account");
        assert_eq!(serialized["fields"]["Name"], "Test");
    }

    #[test]
    fn test_update_record_input_serialize() {
        let mut fields = HashMap::new();
        fields.insert("Phone".to_string(), json!("+1-555-9999"));

        let input = UpdateRecordInput { fields };
        let serialized = serde_json::to_value(&input).must();
        assert_eq!(serialized["fields"]["Phone"], "+1-555-9999");
    }

    #[test]
    fn test_batch_result_representation_deserialize() {
        let json_str = r#"{"hasErrors": false, "results": []}"#;
        let result: BatchResultRepresentation = serde_json::from_str(json_str).must();
        assert!(!result.has_errors);
        assert!(result.results.is_empty());
    }

    #[test]
    fn test_record_defaults_representation_deserialize_plural_object_infos() {
        // Real "Get Record Defaults for Create" shape: `objectInfos` (plural map),
        // `layout`, and `record`. The old singular `objectInfo` key is gone.
        let json_str = r#"{
            "layout": {"sections": []},
            "objectInfos": {
                "Contact": {
                    "apiName": "Contact",
                    "label": "Contact",
                    "labelPlural": "Contacts",
                    "keyPrefix": "003",
                    "fields": {
                        "LastName": {
                            "apiName": "LastName",
                            "label": "Last Name",
                            "dataType": "String",
                            "required": true,
                            "updateable": true,
                            "createable": true,
                            "referenceToInfos": []
                        }
                    }
                }
            },
            "record": {
                "apiName": "Contact",
                "id": null,
                "fields": {},
                "childRelationships": {},
                "recordTypeId": null,
                "systemModstamp": null
            }
        }"#;

        let defaults: RecordDefaultsRepresentation = serde_json::from_str(json_str).must();
        assert_eq!(defaults.record.api_name, "Contact");
        assert!(defaults.object_infos.contains_key("Contact"));
        let info = &defaults.object_infos["Contact"];
        assert_eq!(info.api_name, "Contact");
        assert!(info.fields.contains_key("LastName"));
    }

    #[test]
    fn test_record_ui_representation_deserialize() {
        let json_str = r#"{
            "layoutUserStates": {},
            "layouts": {},
            "objectInfos": {},
            "records": {}
        }"#;
        let ui: RecordUiRepresentation = serde_json::from_str(json_str).must();
        assert!(ui.records.is_empty());
    }
}
