//! GraphQL `uiapi.lookupValues` helper (v68.0+).
//!
//! Gets lookup field suggestions: recent records, type-ahead, or full search.
//! It mirrors the UI API "Get Lookup Field Suggestions With POST" resource.

use super::GraphqlRequest;
use crate::error::{ForceError, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashMap;

/// Default selection set for each `LookupValues` item.
const DEFAULT_SELECTION: &str = "targetApiName records { edges { node { Id DisplayValue } } }";

/// Search mode for `lookupValues`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LookupSearchType {
    /// Most recently used records. Server default.
    Recent,
    /// Full search. Needs `q`.
    Search,
    /// Type-ahead search. Needs `q`.
    TypeAhead,
}

impl LookupSearchType {
    /// Returns the GraphQL enum value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Recent => "RECENT",
            Self::Search => "SEARCH",
            Self::TypeAhead => "TYPE_AHEAD",
        }
    }

    const fn needs_query(self) -> bool {
        matches!(self, Self::Search | Self::TypeAhead)
    }
}

/// Request for `uiapi.lookupValues`.
#[derive(Debug, Clone, PartialEq)]
pub struct LookupValuesRequest {
    /// Object API name, for example `"Opportunity"`.
    pub object_api_name: String,
    /// Lookup field API name, for example `"AccountId"`.
    pub field_api_name: String,
    /// Search text. Needed for `Search` and `TypeAhead`.
    pub q: Option<String>,
    /// Search mode. Server default is `Recent`.
    pub search_type: Option<LookupSearchType>,
    /// Target object for a polymorphic lookup.
    pub target_api_name: Option<String>,
    /// Live field values for dependent lookups. One top-level key: the object API name.
    pub source_record: Option<Value>,
    /// Selection set for each item. `None` uses the default.
    pub selection: Option<String>,
}

impl LookupValuesRequest {
    /// Creates a request for the lookup field `field_api_name` on `object_api_name`.
    #[must_use]
    pub fn new(object_api_name: impl Into<String>, field_api_name: impl Into<String>) -> Self {
        Self {
            object_api_name: object_api_name.into(),
            field_api_name: field_api_name.into(),
            q: None,
            search_type: None,
            target_api_name: None,
            source_record: None,
            selection: None,
        }
    }

    /// Sets the search mode and text.
    ///
    /// The UI API resource needs at least 2 characters for `Search` and 3 for
    /// `TypeAhead`. The server checks this.
    #[must_use]
    pub fn search(mut self, search_type: LookupSearchType, q: impl Into<String>) -> Self {
        self.search_type = Some(search_type);
        self.q = Some(q.into());
        self
    }

    /// Sets the search mode without text (for `Recent`).
    #[must_use]
    pub const fn search_type(mut self, search_type: LookupSearchType) -> Self {
        self.search_type = Some(search_type);
        self
    }

    /// Sets the target object of a polymorphic lookup.
    #[must_use]
    pub fn target_api_name(mut self, target: impl Into<String>) -> Self {
        self.target_api_name = Some(target.into());
        self
    }

    /// Sets the source record snapshot for dependent lookups.
    #[must_use]
    pub fn source_record(mut self, record: Value) -> Self {
        self.source_record = Some(record);
        self
    }

    /// Replaces the default selection set of each item.
    ///
    /// The text goes into the query as is. Use developer-written GraphQL only,
    /// never user input. [`validate`](Self::validate) rejects text that closes
    /// the `lookupValues` block or has a `#` comment.
    #[must_use]
    pub fn selection(mut self, selection: impl Into<String>) -> Self {
        self.selection = Some(selection.into());
        self
    }

    /// Checks the request rules.
    ///
    /// # Errors
    ///
    /// Returns [`ForceError::InvalidInput`] if a name is empty, if `Search` or
    /// `TypeAhead` has no `q`, or if `source_record` is not an object with
    /// exactly one key.
    pub fn validate(&self) -> Result<()> {
        if self.object_api_name.is_empty() || self.field_api_name.is_empty() {
            return Err(ForceError::InvalidInput(
                "lookupValues needs objectApiName and fieldApiName".to_string(),
            ));
        }
        if self.search_type.is_some_and(LookupSearchType::needs_query)
            && self.q.as_deref().is_none_or(str::is_empty)
        {
            return Err(ForceError::InvalidInput(
                "lookupValues SEARCH and TYPE_AHEAD need q".to_string(),
            ));
        }
        if let Some(selection) = &self.selection
            && !is_closed_selection(selection)
        {
            return Err(ForceError::InvalidInput(
                "lookupValues selection must be non-empty, have balanced braces that never \
                 close the outer block, and have no '#'"
                    .to_string(),
            ));
        }
        if let Some(record) = &self.source_record
            && record.as_object().is_none_or(|o| o.len() != 1)
        {
            return Err(ForceError::InvalidInput(
                "lookupValues sourceRecord must have exactly one top-level key".to_string(),
            ));
        }
        Ok(())
    }

    /// Builds the GraphQL request. Only set arguments are sent, so server
    /// defaults apply to the others.
    ///
    /// # Errors
    ///
    /// Returns an error if [`validate`](Self::validate) fails.
    pub fn to_graphql_request(&self) -> Result<GraphqlRequest> {
        self.validate()?;
        let mut decls = vec!["$objectApiName: String!", "$fieldApiName: String!"];
        let mut args = vec![
            "objectApiName: $objectApiName",
            "fieldApiName: $fieldApiName",
        ];
        let mut vars = Map::new();
        vars.insert("objectApiName".into(), self.object_api_name.clone().into());
        vars.insert("fieldApiName".into(), self.field_api_name.clone().into());
        if let Some(q) = &self.q {
            decls.push("$q: String");
            args.push("q: $q");
            vars.insert("q".into(), q.clone().into());
        }
        if let Some(st) = self.search_type {
            decls.push("$searchType: LookupSearchType");
            args.push("searchType: $searchType");
            vars.insert("searchType".into(), st.as_str().into());
        }
        if let Some(t) = &self.target_api_name {
            decls.push("$targetApiName: String");
            args.push("targetApiName: $targetApiName");
            vars.insert("targetApiName".into(), t.clone().into());
        }
        if let Some(r) = &self.source_record {
            decls.push("$sourceRecord: RecordSnapshotInput");
            args.push("sourceRecord: $sourceRecord");
            vars.insert("sourceRecord".into(), r.clone());
        }
        let selection = self.selection.as_deref().unwrap_or(DEFAULT_SELECTION);
        let query = format!(
            "query LookupValues({}) {{ uiapi {{ lookupValues({}) {{ {selection} }} }} }}",
            decls.join(", "),
            args.join(", ")
        );
        Ok(GraphqlRequest::new(query)
            .with_variables(Value::Object(vars))
            .with_operation_name("LookupValues"))
    }
}

/// Returns true if `selection` stays inside its block: non-empty, no `#`,
/// and braces that never go below depth 0 and end at depth 0.
fn is_closed_selection(selection: &str) -> bool {
    if selection.trim().is_empty() || selection.contains('#') {
        return false;
    }
    let mut depth: usize = 0;
    for c in selection.chars() {
        match c {
            '{' => depth += 1,
            '}' => match depth.checked_sub(1) {
                Some(d) => depth = d,
                None => return false,
            },
            _ => {}
        }
    }
    depth == 0
}

/// Record in a lookup result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LookupRecord {
    /// Record ID.
    #[serde(rename = "Id", default)]
    pub id: Option<String>,
    /// Display value, for example the record name.
    #[serde(rename = "DisplayValue", default)]
    pub display_value: Option<String>,
    /// Other selected fields.
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// Edge of a lookup record connection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LookupEdge {
    /// The record.
    pub node: LookupRecord,
}

/// Record connection of one lookup target.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct LookupRecords {
    /// Edges.
    #[serde(default)]
    pub edges: Vec<LookupEdge>,
    /// Other selected fields, for example `pageInfo`.
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// Lookup suggestions for one target object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LookupValues {
    /// Target object API name.
    #[serde(default)]
    pub target_api_name: Option<String>,
    /// Matching records.
    #[serde(default)]
    pub records: LookupRecords,
}

#[derive(Deserialize)]
struct LookupData {
    uiapi: LookupUiapi,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LookupUiapi {
    #[serde(default)]
    lookup_values: Option<Vec<LookupValues>>,
}

impl<A: crate::auth::Authenticator> super::GraphqlHandler<A> {
    /// Gets lookup field suggestions with `uiapi.lookupValues` (v68.0+).
    ///
    /// # Errors
    ///
    /// Returns an error if the request is not valid, the HTTP call fails, or
    /// the response has GraphQL errors and no data.
    pub async fn lookup_values(&self, request: &LookupValuesRequest) -> Result<Vec<LookupValues>> {
        let graphql_request = request.to_graphql_request()?;
        let data: LookupData = self.query(&graphql_request).await?;
        Ok(data.uiapi.lookup_values.unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::builder;
    use crate::test_utils::mock_auth::MockAuthenticator;
    use crate::test_utils::must::Must;
    use serde_json::json;
    use wiremock::matchers::{body_partial_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn test_recent_request_sends_only_set_variables() {
        let req = LookupValuesRequest::new("Opportunity", "AccountId")
            .search_type(LookupSearchType::Recent)
            .to_graphql_request()
            .must();
        let body = serde_json::to_value(&req).must();
        assert_eq!(
            body["variables"],
            json!({"objectApiName": "Opportunity", "fieldApiName": "AccountId", "searchType": "RECENT"})
        );
        let q = body["query"].as_str().must();
        assert!(q.contains("$searchType: LookupSearchType"), "{q}");
        assert!(
            q.contains("lookupValues(objectApiName: $objectApiName"),
            "{q}"
        );
        assert!(!q.contains("$q"), "{q}");
        assert!(q.contains(DEFAULT_SELECTION), "{q}");
    }

    #[test]
    fn test_validate_rules() {
        let base = LookupValuesRequest::new("Opportunity", "AccountId");
        assert!(base.validate().is_ok());
        assert!(
            base.clone()
                .search_type(LookupSearchType::TypeAhead)
                .validate()
                .is_err()
        );
        assert!(
            base.clone()
                .search(LookupSearchType::Search, "")
                .validate()
                .is_err()
        );
        assert!(
            base.clone()
                .source_record(json!({"Opportunity": {}, "Extra": {}}))
                .validate()
                .is_err()
        );
        assert!(base.source_record(json!([1])).validate().is_err());
        assert!(
            LookupValuesRequest::new("", "AccountId")
                .validate()
                .is_err()
        );
    }

    #[test]
    fn test_custom_selection_and_all_arguments() {
        let req = LookupValuesRequest::new("Case", "WhoId")
            .search(LookupSearchType::TypeAhead, "Ac")
            .target_api_name("Contact")
            .source_record(json!({"Case": {"fields": {}}}))
            .selection("targetApiName")
            .to_graphql_request()
            .must();
        let body = serde_json::to_value(&req).must();
        let q = body["query"].as_str().must();
        assert!(q.contains("sourceRecord: $sourceRecord"), "{q}");
        assert!(q.ends_with("{ targetApiName } } }"), "{q}");
        assert_eq!(body["variables"]["targetApiName"], "Contact");
        assert_eq!(body["operationName"], "LookupValues");
    }

    #[test]
    fn test_selection_cannot_escape_lookup_values() {
        let base = LookupValuesRequest::new("Opportunity", "AccountId");
        for bad in [
            "targetApiName } } x: uiapi { query { User { edges { node { Id } } } } } {",
            "targetApiName }",
            "targetApiName # comment",
            "records { edges",
            "",
        ] {
            assert!(base.clone().selection(bad).validate().is_err(), "{bad}");
        }
        assert!(
            base.selection("targetApiName records { edges { node { Id } } }")
                .validate()
                .is_ok()
        );
    }

    #[test]
    fn test_empty_field_name_is_rejected() {
        assert!(
            LookupValuesRequest::new("Opportunity", "")
                .validate()
                .is_err()
        );
    }

    #[test]
    fn test_minimal_query_golden() {
        let req = LookupValuesRequest::new("Opportunity", "AccountId")
            .to_graphql_request()
            .must();
        assert_eq!(
            req.query,
            "query LookupValues($objectApiName: String!, $fieldApiName: String!) { uiapi { \
             lookupValues(objectApiName: $objectApiName, fieldApiName: $fieldApiName) { \
             targetApiName records { edges { node { Id DisplayValue } } } } } }"
        );
    }

    #[tokio::test]
    async fn test_lookup_values_errors_only_response_is_err() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/services/data/v68.0/graphql"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": null,
                "errors": [{"message": "Field 'lookupValues' doesn't exist"}]
            })))
            .mount(&server)
            .await;
        let client = builder()
            .authenticate(MockAuthenticator::new("t", &server.uri()))
            .build()
            .await
            .must();
        let err = client
            .graphql()
            .lookup_values(&LookupValuesRequest::new("Opportunity", "AccountId"))
            .await
            .err()
            .must();
        assert!(err.to_string().contains("lookupValues"), "{err}");
    }

    #[tokio::test]
    async fn test_lookup_values_decodes_response() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/services/data/v68.0/graphql"))
            .and(body_partial_json(json!({
                "operationName": "LookupValues",
                "variables": {"objectApiName": "Opportunity", "q": "Acme", "searchType": "TYPE_AHEAD"}
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": {"uiapi": {"lookupValues": [{
                    "targetApiName": "Account",
                    "records": {"edges": [
                        {"node": {"Id": "001xx000003DHP0AAA", "DisplayValue": "Acme"}}
                    ]}
                }]}}
            })))
            .expect(1)
            .mount(&server)
            .await;
        let client = builder()
            .authenticate(MockAuthenticator::new("t", &server.uri()))
            .build()
            .await
            .must();

        let req = LookupValuesRequest::new("Opportunity", "AccountId")
            .search(LookupSearchType::TypeAhead, "Acme");
        let out = client.graphql().lookup_values(&req).await.must();
        assert_eq!(out[0].target_api_name.as_deref(), Some("Account"));
        assert_eq!(
            out[0].records.edges[0].node.display_value.as_deref(),
            Some("Acme")
        );
    }

    #[tokio::test]
    async fn test_lookup_values_null_list_is_empty() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/services/data/v68.0/graphql"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({"data": {"uiapi": {"lookupValues": null}}})),
            )
            .mount(&server)
            .await;
        let client = builder()
            .authenticate(MockAuthenticator::new("t", &server.uri()))
            .build()
            .await
            .must();

        let out = client
            .graphql()
            .lookup_values(&LookupValuesRequest::new("Opportunity", "AccountId"))
            .await
            .must();
        assert_eq!(out.len(), 0);
    }

    #[tokio::test]
    async fn test_lookup_values_invalid_request_sends_nothing() {
        let server = MockServer::start().await;
        let client = builder()
            .authenticate(MockAuthenticator::new("t", &server.uri()))
            .build()
            .await
            .must();
        let req = LookupValuesRequest::new("Opportunity", "AccountId")
            .search_type(LookupSearchType::Search);
        assert!(client.graphql().lookup_values(&req).await.is_err());
        assert!(server.received_requests().await.must().is_empty());
    }
}
