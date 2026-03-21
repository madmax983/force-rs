//! UI API object info endpoints.
//!
//! Provides object metadata including field descriptions and picklist values
//! via the Salesforce UI API (`/services/data/vXX.0/ui-api/object-info/`).

#![allow(clippy::doc_markdown)]

use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;

// ─── Response types ───────────────────────────────────────────────────────────

/// Metadata about a Salesforce object returned by the UI API.
///
/// Returned by `object_info()`. Contains field descriptions, label
/// information, and the key prefix used to identify records of this type.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectInfoRepresentation {
    /// The SObject API name (e.g., `"Account"`).
    pub api_name: String,
    /// Singular display label.
    pub label: String,
    /// Plural display label.
    pub label_plural: String,
    /// Three-character key prefix that identifies this object type in record IDs.
    pub key_prefix: Option<String>,
    /// Field metadata map keyed by field API name.
    pub fields: HashMap<String, FieldInfoRepresentation>,
    /// Any additional fields returned by the API.
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// Metadata for a single field on a Salesforce object.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldInfoRepresentation {
    /// The field API name (e.g., `"Name"`).
    pub api_name: String,
    /// Human-readable field label.
    pub label: String,
    /// The Salesforce data type string (e.g., `"String"`, `"Picklist"`).
    pub data_type: String,
    /// `true` if this field must have a non-null value on create.
    pub required: bool,
    /// `true` if this field can be updated.
    pub updateable: bool,
    /// `true` if this field can be set on create.
    pub createable: bool,
    /// Related object type information for relationship fields.
    pub reference_to_infos: Vec<ReferenceToInfoRepresentation>,
    /// Any additional fields returned by the API.
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// Information about an object type that a relationship field can reference.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceToInfoRepresentation {
    /// API name of the referenced object type.
    pub api_name: String,
    /// Fields used to display the related record's name.
    pub name_fields: Vec<String>,
    /// Any additional fields returned by the API.
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// Batch result containing metadata for multiple Salesforce objects.
///
/// Returned by `object_infos_batch()`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchObjectInfoRepresentation {
    /// `true` if any individual result contained an error.
    pub has_errors: bool,
    /// Individual result entries (may be `ObjectInfoRepresentation` or error objects).
    pub results: Vec<Value>,
    /// Any additional fields returned by the API.
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

// ─── UiHandler<A> implementation ─────────────────────────────────────────────

impl<A: crate::auth::Authenticator> crate::api::ui::UiHandler<A> {
    /// Returns metadata for a single Salesforce object type.
    ///
    /// Calls `GET /ui-api/object-info/{object}`.
    ///
    /// * `object` – the SObject API name (e.g., `"Account"`).
    ///
    /// # Errors
    ///
    /// Returns an error if the object type is not found or the request fails.
    pub async fn object_info(
        &self,
        object: &str,
    ) -> crate::error::Result<ObjectInfoRepresentation> {
        let path = format!("object-info/{object}");
        self.get(&path, None, "Failed to fetch object info").await
    }

    /// Returns metadata for multiple Salesforce object types in a single request.
    ///
    /// Calls `GET /ui-api/object-info/batch/{objects}` where `objects` is a
    /// comma-separated list of SObject API names.
    ///
    /// * `objects` – one or more SObject API names.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails.
    pub async fn object_infos_batch(
        &self,
        objects: &[&str],
    ) -> crate::error::Result<BatchObjectInfoRepresentation> {
        let joined = objects.join(",");
        let path = format!("object-info/batch/{joined}");
        self.get(&path, None, "Failed to fetch batch object infos")
            .await
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use crate::client::builder;
    use crate::test_support::{MockAuthenticator, Must};
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn make_client(server: &MockServer) -> crate::client::ForceClient<MockAuthenticator> {
        let auth = MockAuthenticator::new("test_token", &server.uri());
        builder().authenticate(auth).build().await.must()
    }

    fn account_object_info_json() -> serde_json::Value {
        json!({
            "apiName": "Account",
            "label": "Account",
            "labelPlural": "Accounts",
            "keyPrefix": "001",
            "fields": {
                "Name": {
                    "apiName": "Name",
                    "label": "Account Name",
                    "dataType": "String",
                    "required": true,
                    "updateable": true,
                    "createable": true,
                    "referenceToInfos": []
                },
                "OwnerId": {
                    "apiName": "OwnerId",
                    "label": "Owner ID",
                    "dataType": "Reference",
                    "required": false,
                    "updateable": true,
                    "createable": true,
                    "referenceToInfos": [
                        {
                            "apiName": "User",
                            "nameFields": ["Name"]
                        }
                    ]
                }
            }
        })
    }

    // ── object_info ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_object_info_success() {
        let server = MockServer::start().await;
        let client = make_client(&server).await;

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/ui-api/object-info/Account"))
            .respond_with(ResponseTemplate::new(200).set_body_json(account_object_info_json()))
            .expect(1)
            .mount(&server)
            .await;

        let info = client.ui().object_info("Account").await.must();

        assert_eq!(info.api_name, "Account");
        assert_eq!(info.label, "Account");
        assert_eq!(info.label_plural, "Accounts");
        assert_eq!(info.key_prefix.as_deref(), Some("001"));
        assert!(info.fields.contains_key("Name"));
        assert!(info.fields.contains_key("OwnerId"));
    }

    #[tokio::test]
    async fn test_object_info_not_found() {
        let server = MockServer::start().await;
        let client = make_client(&server).await;

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/ui-api/object-info/NoSuchObject"))
            .respond_with(ResponseTemplate::new(404).set_body_json(json!([{
                "errorCode": "NOT_FOUND",
                "message": "The requested resource does not exist"
            }])))
            .expect(1)
            .mount(&server)
            .await;

        let result = client.ui().object_info("NoSuchObject").await;
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

    // ── object_infos_batch ───────────────────────────────────────────────────

    #[tokio::test]
    async fn test_object_infos_batch_success() {
        let server = MockServer::start().await;
        let client = make_client(&server).await;

        let response_body = json!({
            "hasErrors": false,
            "results": [
                account_object_info_json(),
                {
                    "apiName": "Contact",
                    "label": "Contact",
                    "labelPlural": "Contacts",
                    "keyPrefix": "003",
                    "fields": {}
                }
            ]
        });

        Mock::given(method("GET"))
            .and(path(
                "/services/data/v60.0/ui-api/object-info/batch/Account,Contact",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
            .expect(1)
            .mount(&server)
            .await;

        let result = client
            .ui()
            .object_infos_batch(&["Account", "Contact"])
            .await
            .must();

        assert!(!result.has_errors);
        assert_eq!(result.results.len(), 2);
    }

    #[tokio::test]
    async fn test_object_infos_batch_error() {
        let server = MockServer::start().await;
        let client = make_client(&server).await;

        Mock::given(method("GET"))
            .and(path(
                "/services/data/v60.0/ui-api/object-info/batch/Account,BadObject",
            ))
            .respond_with(ResponseTemplate::new(400).set_body_json(json!([{
                "errorCode": "INVALID_TYPE",
                "message": "sObject type 'BadObject' is not supported"
            }])))
            .expect(1)
            .mount(&server)
            .await;

        let result = client
            .ui()
            .object_infos_batch(&["Account", "BadObject"])
            .await;
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

    // ── unit / deserialization tests ─────────────────────────────────────────

    #[test]
    fn test_object_info_representation_deserialize() {
        let json_str = r#"{
            "apiName": "Opportunity",
            "label": "Opportunity",
            "labelPlural": "Opportunities",
            "keyPrefix": "006",
            "fields": {
                "Name": {
                    "apiName": "Name",
                    "label": "Opportunity Name",
                    "dataType": "String",
                    "required": true,
                    "updateable": true,
                    "createable": true,
                    "referenceToInfos": []
                }
            }
        }"#;

        let info: ObjectInfoRepresentation = serde_json::from_str(json_str).unwrap();
        assert_eq!(info.api_name, "Opportunity");
        assert_eq!(info.label_plural, "Opportunities");
        assert_eq!(info.key_prefix.as_deref(), Some("006"));
        assert!(info.fields.contains_key("Name"));

        let name_field = &info.fields["Name"];
        assert!(name_field.required);
        assert!(name_field.updateable);
        assert!(name_field.createable);
        assert!(name_field.reference_to_infos.is_empty());
    }

    #[test]
    fn test_field_info_representation_deserialize() {
        let json_str = r#"{
            "apiName": "AccountId",
            "label": "Account ID",
            "dataType": "Reference",
            "required": false,
            "updateable": false,
            "createable": true,
            "referenceToInfos": [
                {
                    "apiName": "Account",
                    "nameFields": ["Name"]
                }
            ]
        }"#;

        let field: FieldInfoRepresentation = serde_json::from_str(json_str).unwrap();
        assert_eq!(field.api_name, "AccountId");
        assert_eq!(field.data_type, "Reference");
        assert!(!field.required);
        assert!(!field.updateable);
        assert!(field.createable);
        assert_eq!(field.reference_to_infos.len(), 1);

        let ref_info = &field.reference_to_infos[0];
        assert_eq!(ref_info.api_name, "Account");
        assert_eq!(ref_info.name_fields, vec!["Name"]);
    }
}
