//! Salesforce SOSL (Salesforce Object Search Language) search API.
//!
//! This module provides types and methods for executing SOSL searches across
//! multiple objects and fields in Salesforce.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Result from a SOSL search query.
///
/// SOSL searches can return results from multiple objects, each with its own
/// set of records.
///
/// # Examples
///
/// ```ignore
/// let results = client.rest()
///     .search("FIND {Acme} IN ALL FIELDS RETURNING Account(Name), Contact(Name)")
///     .await?;
///
/// for record_set in &results.search_records {
///     println!("Object: {}", record_set.attributes.type_);
///     for record in &record_set.records {
///         println!("  ID: {}", record.get("Id").must());
///     }
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    /// Search records grouped by object type.
    pub search_records: Vec<SearchRecords>,
}

/// Search results for a specific object type.
///
/// Contains the object type information and all matching records.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchRecords {
    /// Object type metadata.
    #[serde(rename = "attributes")]
    pub attributes: SearchAttributes,

    /// Matching records for this object type.
    pub records: Vec<HashMap<String, serde_json::Value>>,
}

/// Attributes describing the object type in search results.
///
/// Similar to regular SObject attributes but specific to search results.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchAttributes {
    /// Type of the object (e.g., "Account", "Contact").
    #[serde(rename = "type")]
    pub type_: String,

    /// URL for accessing this object type.
    pub url: String,
}

/// Builder for constructing SOSL search queries.
///
/// Provides a fluent API for building SOSL queries with proper escaping
/// and formatting.
///
/// # Examples
///
/// ```
/// use force::api::rest::search::SearchQueryBuilder;
///
/// let query = SearchQueryBuilder::new()
///     .find("Acme Corporation")
///     .in_name_fields()
///     .returning("Account", &["Id", "Name", "Industry"])
///     .limit(10)
///     .build();
///
/// assert_eq!(
///     query,
///     "FIND {Acme Corporation} IN NAME FIELDS RETURNING Account(Id, Name, Industry) LIMIT 10"
/// );
/// ```
#[derive(Debug, Clone)]
pub struct SearchQueryBuilder {
    /// Search text.
    search_text: String,
    /// Search scope (e.g., "ALL FIELDS", "NAME FIELDS").
    search_scope: Option<String>,
    /// Objects and fields to return.
    returning: Vec<(String, Vec<String>)>,
    /// Maximum number of records per object.
    limit: Option<u32>,
    /// Offset for pagination.
    offset: Option<u32>,
}

impl SearchQueryBuilder {
    /// Creates a new search query builder.
    #[must_use]
    pub fn new() -> Self {
        Self {
            search_text: String::new(),
            search_scope: None,
            returning: Vec::new(),
            limit: None,
            offset: None,
        }
    }

    /// Sets the search text.
    ///
    /// The text will be automatically escaped for SOSL.
    ///
    /// # Arguments
    ///
    /// * `text` - The search text
    #[must_use]
    pub fn find(mut self, text: impl Into<String>) -> Self {
        self.search_text = escape_sosl(&text.into());
        self
    }

    /// Searches in all fields.
    #[must_use]
    pub fn in_all_fields(mut self) -> Self {
        self.search_scope = Some("ALL FIELDS".to_string());
        self
    }

    /// Searches in name fields only.
    #[must_use]
    pub fn in_name_fields(mut self) -> Self {
        self.search_scope = Some("NAME FIELDS".to_string());
        self
    }

    /// Searches in email fields only.
    #[must_use]
    pub fn in_email_fields(mut self) -> Self {
        self.search_scope = Some("EMAIL FIELDS".to_string());
        self
    }

    /// Searches in phone fields only.
    #[must_use]
    pub fn in_phone_fields(mut self) -> Self {
        self.search_scope = Some("PHONE FIELDS".to_string());
        self
    }

    /// Searches in sidebar fields only.
    #[must_use]
    pub fn in_sidebar_fields(mut self) -> Self {
        self.search_scope = Some("SIDEBAR FIELDS".to_string());
        self
    }

    /// Adds an object type to return with specific fields.
    ///
    /// # Arguments
    ///
    /// * `sobject` - The object type (e.g., "Account", "Contact")
    /// * `fields` - The fields to return (e.g., `&["Id", "Name"]`)
    #[must_use]
    pub fn returning(mut self, sobject: impl Into<String>, fields: &[impl AsRef<str>]) -> Self {
        let sobject = sobject.into();
        let fields = fields.iter().map(|f| f.as_ref().to_string()).collect();
        self.returning.push((sobject, fields));
        self
    }

    /// Sets the maximum number of records to return per object.
    #[must_use]
    pub fn limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Sets the offset for pagination.
    #[must_use]
    pub fn offset(mut self, offset: u32) -> Self {
        self.offset = Some(offset);
        self
    }

    /// Builds the SOSL query string.
    ///
    /// # Panics
    ///
    /// Panics if search text is empty or no objects are specified in RETURNING.
    #[must_use]
    pub fn build(self) -> String {
        assert!(!self.search_text.is_empty(), "search text cannot be empty");
        assert!(
            !self.returning.is_empty(),
            "at least one object must be specified in RETURNING"
        );

        let mut query = format!("FIND {{{}}}", self.search_text);

        if let Some(scope) = self.search_scope {
            query.push_str(&format!(" IN {}", scope));
        }

        query.push_str(" RETURNING ");
        let returning_parts: Vec<String> = self
            .returning
            .into_iter()
            .map(|(sobject, fields)| {
                if fields.is_empty() {
                    sobject
                } else {
                    format!("{}({})", sobject, fields.join(", "))
                }
            })
            .collect();
        query.push_str(&returning_parts.join(", "));

        if let Some(limit) = self.limit {
            query.push_str(&format!(" LIMIT {}", limit));
        }

        if let Some(offset) = self.offset {
            query.push_str(&format!(" OFFSET {}", offset));
        }

        query
    }
}

impl Default for SearchQueryBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Escapes special characters for SOSL search queries.
///
/// Reserved characters: ? & | ! { } [ ] ( ) ^ ~ * : \ " ' + -
fn escape_sosl(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '?' | '&' | '|' | '!' | '{' | '}' | '[' | ']' | '(' | ')' | '^' | '~' | '*'
            | ':' | '\\' | '"' | '\'' | '+' | '-' => {
                escaped.push('\\');
                escaped.push(c);
            }
            _ => escaped.push(c),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Must;

    // RED PHASE - Write failing tests first

    #[test]
    fn test_search_result_deserialize() {
        let json = r#"{
            "searchRecords": [
                {
                    "attributes": {
                        "type": "Account",
                        "url": "/services/data/v60.0/sobjects/Account/001000000000001AAA"
                    },
                    "records": [
                        {
                            "Id": "001000000000001AAA",
                            "Name": "Acme Corporation"
                        }
                    ]
                }
            ]
        }"#;

        let result: SearchResult = serde_json::from_str(json).must();
        assert_eq!(result.search_records.len(), 1);
        assert_eq!(result.search_records[0].attributes.type_, "Account");
        assert_eq!(result.search_records[0].records.len(), 1);
    }

    #[test]
    fn test_search_result_multiple_objects() {
        let json = r#"{
            "searchRecords": [
                {
                    "attributes": {
                        "type": "Account",
                        "url": "/services/data/v60.0/sobjects/Account"
                    },
                    "records": [
                        {"Id": "001000000000001AAA", "Name": "Acme"}
                    ]
                },
                {
                    "attributes": {
                        "type": "Contact",
                        "url": "/services/data/v60.0/sobjects/Contact"
                    },
                    "records": [
                        {"Id": "003000000000001AAA", "Name": "John Doe"}
                    ]
                }
            ]
        }"#;

        let result: SearchResult = serde_json::from_str(json).must();
        assert_eq!(result.search_records.len(), 2);
        assert_eq!(result.search_records[0].attributes.type_, "Account");
        assert_eq!(result.search_records[1].attributes.type_, "Contact");
    }

    #[test]
    fn test_search_result_empty_records() {
        let json = r#"{
            "searchRecords": []
        }"#;

        let result: SearchResult = serde_json::from_str(json).must();
        assert_eq!(result.search_records.len(), 0);
    }

    #[test]
    fn test_search_query_builder_basic() {
        let query = SearchQueryBuilder::new()
            .find("Acme")
            .in_all_fields()
            .returning("Account", &["Id", "Name"])
            .build();

        assert_eq!(
            query,
            "FIND {Acme} IN ALL FIELDS RETURNING Account(Id, Name)"
        );
    }

    #[test]
    fn test_search_query_builder_multiple_objects() {
        let query = SearchQueryBuilder::new()
            .find("John")
            .in_name_fields()
            .returning("Account", &["Id", "Name"])
            .returning("Contact", &["Id", "FirstName", "LastName"])
            .build();

        assert_eq!(
            query,
            "FIND {John} IN NAME FIELDS RETURNING Account(Id, Name), Contact(Id, FirstName, LastName)"
        );
    }

    #[test]
    fn test_search_query_builder_with_limit() {
        let query = SearchQueryBuilder::new()
            .find("Test")
            .in_all_fields()
            .returning("Account", &["Id"])
            .limit(5)
            .build();

        assert_eq!(
            query,
            "FIND {Test} IN ALL FIELDS RETURNING Account(Id) LIMIT 5"
        );
    }

    #[test]
    fn test_search_query_builder_with_offset() {
        let query = SearchQueryBuilder::new()
            .find("Test")
            .in_all_fields()
            .returning("Account", &["Id"])
            .offset(10)
            .build();

        assert_eq!(
            query,
            "FIND {Test} IN ALL FIELDS RETURNING Account(Id) OFFSET 10"
        );
    }

    #[test]
    fn test_search_query_builder_with_limit_and_offset() {
        let query = SearchQueryBuilder::new()
            .find("Test")
            .in_all_fields()
            .returning("Account", &["Id"])
            .limit(5)
            .offset(10)
            .build();

        assert_eq!(
            query,
            "FIND {Test} IN ALL FIELDS RETURNING Account(Id) LIMIT 5 OFFSET 10"
        );
    }

    #[test]
    fn test_search_query_builder_email_fields() {
        let query = SearchQueryBuilder::new()
            .find("test@example.com")
            .in_email_fields()
            .returning("Contact", &["Id", "Email"])
            .build();

        assert_eq!(
            query,
            "FIND {test@example.com} IN EMAIL FIELDS RETURNING Contact(Id, Email)"
        );
    }

    #[test]
    fn test_search_query_builder_phone_fields() {
        let query = SearchQueryBuilder::new()
            .find("415-555-0100")
            .in_phone_fields()
            .returning("Contact", &["Id", "Phone"])
            .build();

        assert_eq!(
            query,
            r"FIND {415\-555\-0100} IN PHONE FIELDS RETURNING Contact(Id, Phone)"
        );
    }

    #[test]
    fn test_search_query_builder_no_fields() {
        let query = SearchQueryBuilder::new()
            .find("Test")
            .returning("Account", &[] as &[&str])
            .build();

        // When no fields specified, just return the object name
        assert_eq!(query, "FIND {Test} RETURNING Account");
    }

    #[test]
    #[should_panic(expected = "search text cannot be empty")]
    fn test_search_query_builder_empty_text() {
        let _ = SearchQueryBuilder::new()
            .find("")
            .returning("Account", &["Id"])
            .build();
    }

    #[test]
    #[should_panic(expected = "at least one object must be specified")]
    fn test_search_query_builder_no_returning() {
        let _ = SearchQueryBuilder::new().find("Test").build();
    }

    #[test]
    fn test_search_query_builder_escaping() {
        let query = SearchQueryBuilder::new()
            .find(r#"? & | ! { } [ ] ( ) ^ ~ * : \ " ' + -"#)
            .returning("Account", &["Id"])
            .build();

        // All special characters should be escaped with backslash
        let expected = r#"FIND {\? \& \| \! \{ \} \[ \] \( \) \^ \~ \* \: \\ \" \' \+ \-} RETURNING Account(Id)"#;
        assert_eq!(query, expected);
    }
}

// Integration tests with wiremock
#[cfg(all(test, feature = "mock"))]
mod integration_tests {
    use super::*;
    use crate::auth::{AccessToken, Authenticator, TokenResponse};
    use crate::client::builder;
    use crate::config::ClientConfigBuilder;
    use crate::error::Result;
    use crate::test_support::MustMsg;
    use async_trait::async_trait;
    use wiremock::matchers::{bearer_token, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    // Mock authenticator for integration tests
    #[derive(Debug, Clone)]
    struct MockAuthenticator {
        token: String,
        instance_url: String,
    }

    impl MockAuthenticator {
        fn new(token: &str, instance_url: &str) -> Self {
            Self {
                token: token.to_string(),
                instance_url: instance_url.to_string(),
            }
        }
    }

    #[async_trait]
    impl Authenticator for MockAuthenticator {
        async fn authenticate(&self) -> Result<AccessToken> {
            Ok(AccessToken::from_response(TokenResponse {
                access_token: self.token.clone(),
                instance_url: self.instance_url.clone(),
                token_type: "Bearer".to_string(),
                issued_at: "1704067200000".to_string(),
                signature: "test_sig".to_string(),
                expires_in: Some(7200),
                refresh_token: None,
            }))
        }

        async fn refresh(&self) -> Result<AccessToken> {
            self.authenticate().await
        }
    }

    fn sample_search_response() -> serde_json::Value {
        serde_json::json!({
            "searchRecords": [
                {
                    "attributes": {
                        "type": "Account",
                        "url": "/services/data/v60.0/sobjects/Account/001000000000001AAA"
                    },
                    "records": [
                        {
                            "Id": "001000000000001AAA",
                            "Name": "Acme Corporation",
                            "Industry": "Technology"
                        },
                        {
                            "Id": "001000000000002AAA",
                            "Name": "Acme Industries",
                            "Industry": "Manufacturing"
                        }
                    ]
                },
                {
                    "attributes": {
                        "type": "Contact",
                        "url": "/services/data/v60.0/sobjects/Contact/003000000000001AAA"
                    },
                    "records": [
                        {
                            "Id": "003000000000001AAA",
                            "FirstName": "John",
                            "LastName": "Acme"
                        }
                    ]
                }
            ]
        })
    }

    #[tokio::test]
    async fn test_search_success() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());

        let sosl = "FIND {Acme} IN ALL FIELDS RETURNING Account(Id, Name), Contact(Id, Name)";

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/search"))
            .and(query_param("q", sosl))
            .and(bearer_token("test_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(sample_search_response()))
            .mount(&mock_server)
            .await;

        let client = builder()
            .authenticate(auth)
            .build()
            .await
            .must_msg("Failed to build client");

        let results = client.rest().search(sosl).await.must_msg("Search failed");

        assert_eq!(results.search_records.len(), 2);
        assert_eq!(results.search_records[0].attributes.type_, "Account");
        assert_eq!(results.search_records[0].records.len(), 2);
        assert_eq!(results.search_records[1].attributes.type_, "Contact");
        assert_eq!(results.search_records[1].records.len(), 1);
    }

    #[tokio::test]
    async fn test_search_with_builder() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());

        let query = SearchQueryBuilder::new()
            .find("Acme")
            .in_name_fields()
            .returning("Account", &["Id", "Name"])
            .limit(10)
            .build();

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/search"))
            .and(query_param("q", query.as_str()))
            .respond_with(ResponseTemplate::new(200).set_body_json(sample_search_response()))
            .mount(&mock_server)
            .await;

        let client = builder()
            .authenticate(auth)
            .build()
            .await
            .must_msg("Failed to build client");

        let results = client.rest().search(&query).await.must_msg("Search failed");
        assert_eq!(results.search_records.len(), 2);
    }

    #[tokio::test]
    async fn test_search_empty_results() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());

        let empty_response = serde_json::json!({
            "searchRecords": []
        });

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(empty_response))
            .mount(&mock_server)
            .await;

        let client = builder()
            .authenticate(auth)
            .build()
            .await
            .must_msg("Failed to build client");

        let results = client
            .rest()
            .search("FIND {NonExistent} RETURNING Account(Id)")
            .await
            .must_msg("Search failed");

        assert_eq!(results.search_records.len(), 0);
    }

    #[tokio::test]
    async fn test_search_unauthorized() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("invalid_token", &mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/search"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&mock_server)
            .await;

        let client = builder()
            .authenticate(auth)
            .build()
            .await
            .must_msg("Failed to build client");

        let result = client
            .rest()
            .search("FIND {Test} RETURNING Account(Id)")
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_search_malformed_query() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/search"))
            .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
                "message": "Malformed SOSL query",
                "errorCode": "MALFORMED_QUERY"
            })))
            .mount(&mock_server)
            .await;

        let client = builder()
            .authenticate(auth)
            .build()
            .await
            .must_msg("Failed to build client");

        let result = client.rest().search("INVALID SOSL").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_search_single_object() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());

        let single_object_response = serde_json::json!({
            "searchRecords": [
                {
                    "attributes": {
                        "type": "Account",
                        "url": "/services/data/v60.0/sobjects/Account"
                    },
                    "records": [
                        {"Id": "001000000000001AAA", "Name": "Test Account"}
                    ]
                }
            ]
        });

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(single_object_response))
            .mount(&mock_server)
            .await;

        let client = builder()
            .authenticate(auth)
            .build()
            .await
            .must_msg("Failed to build client");

        let results = client
            .rest()
            .search("FIND {Test} RETURNING Account(Id, Name)")
            .await
            .must_msg("Search failed");

        assert_eq!(results.search_records.len(), 1);
        assert_eq!(results.search_records[0].attributes.type_, "Account");
    }

    #[tokio::test]
    async fn test_search_with_custom_api_version() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("custom_token", &mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/services/data/v59.0/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(sample_search_response()))
            .mount(&mock_server)
            .await;

        let config = ClientConfigBuilder::new().api_version("v59.0").build();
        let client = builder()
            .authenticate(auth)
            .config(config)
            .build()
            .await
            .must_msg("Failed to build client");

        let results = client
            .rest()
            .search("FIND {Acme} RETURNING Account(Id)")
            .await
            .must_msg("Search failed");

        assert_eq!(results.search_records.len(), 2);
    }

    #[tokio::test]
    async fn test_search_email_fields() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());

        let query = SearchQueryBuilder::new()
            .find("test@example.com")
            .in_email_fields()
            .returning("Contact", &["Id", "Email"])
            .build();

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/search"))
            .and(query_param(
                "q",
                "FIND {test@example.com} IN EMAIL FIELDS RETURNING Contact(Id, Email)",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(sample_search_response()))
            .mount(&mock_server)
            .await;

        let client = builder()
            .authenticate(auth)
            .build()
            .await
            .must_msg("Failed to build client");

        client.rest().search(&query).await.must_msg("Search failed");
    }

    #[tokio::test]
    async fn test_search_phone_fields() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());

        let query = SearchQueryBuilder::new()
            .find("415-555-0100")
            .in_phone_fields()
            .returning("Contact", &["Id", "Phone"])
            .build();

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(sample_search_response()))
            .mount(&mock_server)
            .await;

        let client = builder()
            .authenticate(auth)
            .build()
            .await
            .must_msg("Failed to build client");

        client.rest().search(&query).await.must_msg("Search failed");
    }

    #[tokio::test]
    async fn test_search_with_limit_and_offset() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());

        let query = SearchQueryBuilder::new()
            .find("Acme")
            .in_all_fields()
            .returning("Account", &["Id"])
            .limit(10)
            .offset(20)
            .build();

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/search"))
            .and(query_param(
                "q",
                "FIND {Acme} IN ALL FIELDS RETURNING Account(Id) LIMIT 10 OFFSET 20",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(sample_search_response()))
            .mount(&mock_server)
            .await;

        let client = builder()
            .authenticate(auth)
            .build()
            .await
            .must_msg("Failed to build client");

        client.rest().search(&query).await.must_msg("Search failed");
    }

    #[tokio::test]
    async fn test_search_multiple_calls() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(sample_search_response()))
            .expect(3)
            .mount(&mock_server)
            .await;

        let client = builder()
            .authenticate(auth)
            .build()
            .await
            .must_msg("Failed to build client");

        for _ in 0..3 {
            let results = client
                .rest()
                .search("FIND {Test} RETURNING Account(Id)")
                .await
                .must_msg("Search failed");
            assert_eq!(results.search_records.len(), 2);
        }
    }

    #[tokio::test]
    async fn test_search_cloned_handler() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(sample_search_response()))
            .expect(2)
            .mount(&mock_server)
            .await;

        let client = builder()
            .authenticate(auth)
            .build()
            .await
            .must_msg("Failed to build client");

        let handler1 = client.rest();
        let handler2 = handler1.clone();

        let results1 = handler1
            .search("FIND {Test} RETURNING Account(Id)")
            .await
            .must_msg("Handler1 search failed");
        let results2 = handler2
            .search("FIND {Test} RETURNING Account(Id)")
            .await
            .must_msg("Handler2 search failed");

        assert_eq!(results1.search_records.len(), results2.search_records.len());
    }
}
