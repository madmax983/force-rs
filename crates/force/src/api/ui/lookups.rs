//! UI API lookup endpoints.
//!
//! Provides type-ahead lookup search for reference fields via the
//! Salesforce UI API (`/services/data/vXX.0/ui-api/lookups/{object}/{field}`).

#![allow(clippy::doc_markdown)]

use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;

// ─── Response types ──────────────────────────────────────────────────────────

/// The results of a lookup search.
///
/// Returned by `lookup()` and `filtered_lookup()`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LookupResultsRepresentation {
    /// Total number of matching results.
    pub count: u32,
    /// The individual lookup result entries.
    pub lookup_results: Vec<LookupResult>,
    /// Any additional fields returned by the API.
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// A single lookup result entry.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LookupResult {
    /// The Salesforce record ID.
    pub id: String,
    /// The primary display label (e.g., record name).
    pub label: String,
    /// An optional secondary label (e.g., account name for contacts).
    pub sublabel: Option<String>,
    /// The SObject API name.
    pub api_name: String,
    /// Any additional fields returned by the API.
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

// ─── UiHandler<A> implementation ─────────────────────────────────────────────

impl<A: crate::auth::Authenticator> crate::api::ui::UiHandler<A> {
    /// Searches for records matching a type-ahead query for a lookup field.
    ///
    /// Calls `GET /ui-api/lookups/{object}/{field}?q={query}`.
    ///
    /// * `object` – SObject API name (e.g., `"Contact"`).
    /// * `field` – Field API name of the lookup field (e.g., `"AccountId"`).
    /// * `query` – The search string entered by the user.
    ///
    /// # Errors
    ///
    /// Returns an error if the object or field is invalid, or the request fails.
    pub async fn lookup(
        &self,
        object: &str,
        field: &str,
        query: &str,
    ) -> crate::error::Result<LookupResultsRepresentation> {
        crate::types::validator::validate_sobject_name(object)?;
        crate::types::validator::validate_field_name(field)?;

        let path = format!("lookups/{object}/{field}");
        let params = &[("q", query)];
        self.get(&path, Some(params), "Failed to perform lookup")
            .await
    }

    /// Searches for records matching a type-ahead query for a lookup field,
    /// restricted to records of the given target SObject type.
    ///
    /// Calls `GET /ui-api/lookups/{object}/{field}/{target}?q={query}`.
    ///
    /// * `object` – SObject API name of the source object.
    /// * `field` – Field API name of the lookup field.
    /// * `target` – SObject API name of the target (restricts result type).
    /// * `query` – The search string entered by the user.
    ///
    /// # Errors
    ///
    /// Returns an error if the object, field, or target is invalid, or the
    /// request fails.
    pub async fn filtered_lookup(
        &self,
        object: &str,
        field: &str,
        target: &str,
        query: &str,
    ) -> crate::error::Result<LookupResultsRepresentation> {
        crate::types::validator::validate_sobject_name(object)?;
        crate::types::validator::validate_field_name(field)?;
        crate::types::validator::validate_sobject_name(target)?;

        let path = format!("lookups/{object}/{field}/{target}");
        let params = &[("q", query)];
        self.get(&path, Some(params), "Failed to perform filtered lookup")
            .await
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::builder;
    use crate::test_support::{MockAuthenticator, Must};
    use serde_json::json;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn make_client(server: &MockServer) -> crate::client::ForceClient<MockAuthenticator> {
        let auth = MockAuthenticator::new("test_token", &server.uri());
        builder().authenticate(auth).build().await.must()
    }

    fn lookup_result_json(id: &str, label: &str) -> serde_json::Value {
        json!({
            "id": id,
            "label": label,
            "sublabel": null,
            "apiName": "Account"
        })
    }

    // ── lookup success with results ───────────────────────────────────────────

    #[tokio::test]
    async fn test_lookup_success_with_results() {
        let server = MockServer::start().await;
        let client = make_client(&server).await;

        let response_body = json!({
            "count": 2,
            "lookupResults": [
                lookup_result_json("001000000000001AAA", "Acme Corp"),
                lookup_result_json("001000000000002AAA", "Acme Ltd")
            ]
        });

        Mock::given(method("GET"))
            .and(path(
                "/services/data/v60.0/ui-api/lookups/Opportunity/AccountId",
            ))
            .and(query_param("q", "Acme"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
            .expect(1)
            .mount(&server)
            .await;

        let result = client
            .ui()
            .lookup("Opportunity", "AccountId", "Acme")
            .await
            .must();

        assert_eq!(result.count, 2);
        assert_eq!(result.lookup_results.len(), 2);
        assert_eq!(result.lookup_results[0].label, "Acme Corp");
    }

    // ── lookup empty results ──────────────────────────────────────────────────

    #[tokio::test]
    async fn test_lookup_empty_results() {
        let server = MockServer::start().await;
        let client = make_client(&server).await;

        let response_body = json!({
            "count": 0,
            "lookupResults": []
        });

        Mock::given(method("GET"))
            .and(path(
                "/services/data/v60.0/ui-api/lookups/Contact/AccountId",
            ))
            .and(query_param("q", "ZZZnonexistent"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
            .expect(1)
            .mount(&server)
            .await;

        let result = client
            .ui()
            .lookup("Contact", "AccountId", "ZZZnonexistent")
            .await
            .must();

        assert_eq!(result.count, 0);
        assert!(result.lookup_results.is_empty());
    }

    // ── filtered_lookup success ───────────────────────────────────────────────

    #[tokio::test]
    async fn test_filtered_lookup_success() {
        let server = MockServer::start().await;
        let client = make_client(&server).await;

        let response_body = json!({
            "count": 1,
            "lookupResults": [
                lookup_result_json("001000000000001AAA", "Global Industries")
            ]
        });

        Mock::given(method("GET"))
            .and(path(
                "/services/data/v60.0/ui-api/lookups/Contact/AccountId/Account",
            ))
            .and(query_param("q", "Global"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
            .expect(1)
            .mount(&server)
            .await;

        let result = client
            .ui()
            .filtered_lookup("Contact", "AccountId", "Account", "Global")
            .await
            .must();

        assert_eq!(result.count, 1);
        assert_eq!(result.lookup_results[0].label, "Global Industries");
    }

    // ── filtered_lookup error case ────────────────────────────────────────────

    #[tokio::test]
    async fn test_filtered_lookup_invalid_object_error() {
        let server = MockServer::start().await;
        let client = make_client(&server).await;

        Mock::given(method("GET"))
            .and(path(
                "/services/data/v60.0/ui-api/lookups/NoSuchObject/SomeField/Account",
            ))
            .and(query_param("q", "test"))
            .respond_with(ResponseTemplate::new(404).set_body_json(json!([{
                "errorCode": "NOT_FOUND",
                "message": "The requested resource does not exist"
            }])))
            .expect(1)
            .mount(&server)
            .await;

        let result = client
            .ui()
            .filtered_lookup("NoSuchObject", "SomeField", "Account", "test")
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

    // ── unit: deserialize LookupResultsRepresentation ─────────────────────────

    #[test]
    fn test_lookup_results_representation_deserialize() {
        let json_str = r#"{
            "count": 1,
            "lookupResults": [
                {
                    "id": "003000000000001AAA",
                    "label": "Jane Doe",
                    "sublabel": "Acme Corp",
                    "apiName": "Contact"
                }
            ]
        }"#;

        let rep: LookupResultsRepresentation = serde_json::from_str(json_str).must();
        assert_eq!(rep.count, 1);
        assert_eq!(rep.lookup_results.len(), 1);
        let entry = &rep.lookup_results[0];
        assert_eq!(entry.id, "003000000000001AAA");
        assert_eq!(entry.label, "Jane Doe");
        assert_eq!(entry.sublabel.as_deref(), Some("Acme Corp"));
        assert_eq!(entry.api_name, "Contact");
    }

    #[tokio::test]
    async fn test_lookup_invalid_sobject_name() {
        let server = MockServer::start().await;
        let client = make_client(&server).await;

        let result = client
            .ui()
            .lookup("Account; DROP TABLE", "AccountId", "test")
            .await;
        assert!(result.is_err());
        let Err(e) = result else {
            panic!("Expected error");
        };
        assert!(
            e.to_string()
                .contains("SObject name contains invalid characters")
        );
    }

    #[tokio::test]
    async fn test_lookup_invalid_field_name() {
        let server = MockServer::start().await;
        let client = make_client(&server).await;

        let result = client
            .ui()
            .lookup("Account", "AccountId; DROP TABLE", "test")
            .await;
        assert!(result.is_err());
        let Err(e) = result else {
            panic!("Expected error");
        };
        assert!(
            e.to_string()
                .contains("Field name contains invalid character")
        );
    }

    #[tokio::test]
    async fn test_filtered_lookup_invalid_sobject_name() {
        let server = MockServer::start().await;
        let client = make_client(&server).await;

        let result = client
            .ui()
            .filtered_lookup("Account; DROP TABLE", "AccountId", "Contact", "test")
            .await;
        assert!(result.is_err());
        let Err(e) = result else {
            panic!("Expected error");
        };
        assert!(
            e.to_string()
                .contains("SObject name contains invalid characters")
        );
    }

    #[tokio::test]
    async fn test_filtered_lookup_invalid_field_name() {
        let server = MockServer::start().await;
        let client = make_client(&server).await;

        let result = client
            .ui()
            .filtered_lookup("Account", "AccountId; DROP TABLE", "Contact", "test")
            .await;
        assert!(result.is_err());
        let Err(e) = result else {
            panic!("Expected error");
        };
        assert!(
            e.to_string()
                .contains("Field name contains invalid character")
        );
    }
}
