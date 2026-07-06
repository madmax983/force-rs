//! Salesforce SOSL (Salesforce Object Search Language) search API.
//!
//! This module provides types and methods for executing SOSL searches across
//! multiple objects and fields in Salesforce.

use crate::api::builder_unwrap::BuilderUnwrapExt;
use crate::types::validator::validate_sobject_name;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
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
/// use force::api::rest::SearchQueryBuilder;
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
    /// ⚡ Bolt: Using `&'static str` for predefined search scopes avoids `.to_string()` heap allocations.
    /// Search scope (e.g., "ALL FIELDS", "NAME FIELDS").
    search_scope: Option<&'static str>,
    /// Objects and fields to return.
    returning: Vec<(String, String)>,
    /// Maximum number of records per object.
    limit: Option<u32>,
    /// Offset for pagination.
    offset: Option<u32>,
    /// Deferred validation error.
    error: Option<String>,
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
            error: None,
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
    pub fn find(mut self, text: &str) -> Self {
        self.search_text = escape_sosl(text).into_owned();
        self
    }

    /// Searches in all fields.
    #[must_use]
    pub fn in_all_fields(mut self) -> Self {
        self.search_scope = Some("ALL FIELDS");
        self
    }

    /// Searches in name fields only.
    #[must_use]
    pub fn in_name_fields(mut self) -> Self {
        self.search_scope = Some("NAME FIELDS");
        self
    }

    /// Searches in email fields only.
    #[must_use]
    pub fn in_email_fields(mut self) -> Self {
        self.search_scope = Some("EMAIL FIELDS");
        self
    }

    /// Searches in phone fields only.
    #[must_use]
    pub fn in_phone_fields(mut self) -> Self {
        self.search_scope = Some("PHONE FIELDS");
        self
    }

    /// Searches in sidebar fields only.
    #[must_use]
    pub fn in_sidebar_fields(mut self) -> Self {
        self.search_scope = Some("SIDEBAR FIELDS");
        self
    }

    /// Adds an object type to return with specific fields.
    ///
    /// # Arguments
    ///
    /// * `sobject` - The object type (e.g., "Account", "Contact")
    /// * `fields` - The fields to return (e.g., `&["Id", "Name"]`)
    ///
    /// # Errors
    ///
    /// Returns an error if object names contain non-alphanumeric/underscore characters or
    /// if field names contain characters other than alphanumeric, underscores, or dots.
    pub fn try_returning(
        mut self,
        sobject: impl Into<String>,
        fields: &[impl AsRef<str>],
    ) -> crate::error::Result<Self> {
        let sobject = sobject.into();
        validate_sobject_name(&sobject)?;

        #[allow(unused_doc_comments)]
        /// ⚡ Bolt: Join fields into a single string immediately to avoid storing `Vec<String>` allocations per object type.
        let mut fields_str = String::with_capacity(fields.len() * 16);
        for (i, f) in fields.iter().enumerate() {
            let f_str = f.as_ref();
            validate_field_syntax_safe(f_str).map_err(crate::error::ForceError::InvalidInput)?;
            if i > 0 {
                fields_str.push_str(", ");
            }
            fields_str.push_str(f_str);
        }

        self.returning.push((sobject, fields_str));
        Ok(self)
    }

    /// Adds an object type to return with specific fields.
    ///
    /// # Arguments
    ///
    /// * `sobject` - The object type (e.g., "Account", "Contact")
    /// * `fields` - The fields to return (e.g., `&["Id", "Name"]`)
    ///
    /// # Panics
    ///
    /// Panics if object names contain non-alphanumeric/underscore characters or
    /// if field names contain characters other than alphanumeric, underscores, or dots.
    #[must_use]
    pub fn returning(mut self, sobject: impl Into<String>, fields: &[impl AsRef<str>]) -> Self {
        let sobject = sobject.into();
        match self.clone().try_returning(sobject, fields) {
            Ok(s) => s,
            Err(e) => {
                self.error = Some(e.to_string());
                self
            }
        }
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

    /// Builds the SOSL query string (non-panicking version).
    ///
    /// # Errors
    ///
    /// Returns an error if search text is empty or no objects are specified in RETURNING.
    pub fn try_build(self) -> crate::error::Result<String> {
        use std::fmt::Write;

        if let Some(err) = self.error {
            return Err(crate::error::ForceError::InvalidInput(err));
        }

        if self.search_text.is_empty() {
            return Err(crate::error::ForceError::InvalidInput(
                "search text cannot be empty".to_string(),
            ));
        }

        if self.returning.is_empty() {
            return Err(crate::error::ForceError::InvalidInput(
                "at least one object must be specified in RETURNING".to_string(),
            ));
        }

        let mut query = String::with_capacity(128);

        write!(&mut query, "FIND {{{}}}", self.search_text)
            .unwrap_or_else(|_| unreachable!("String format cannot fail"));

        if let Some(scope) = self.search_scope {
            write!(&mut query, " IN {}", scope)
                .unwrap_or_else(|_| unreachable!("String format cannot fail"));
        }

        query.push_str(" RETURNING ");

        // ⚡ Bolt: Write RETURNING clauses directly to the `query` buffer.
        // `fields` is already a comma-separated string, avoiding intermediate vector iteration.
        for (i, (sobject, fields)) in self.returning.into_iter().enumerate() {
            if i > 0 {
                query.push_str(", ");
            }

            query.push_str(&sobject);
            if !fields.is_empty() {
                query.push('(');
                query.push_str(&fields);
                query.push(')');
            }
        }

        if let Some(limit) = self.limit {
            write!(&mut query, " LIMIT {}", limit)
                .unwrap_or_else(|_| unreachable!("String format cannot fail"));
        }

        if let Some(offset) = self.offset {
            write!(&mut query, " OFFSET {}", offset)
                .unwrap_or_else(|_| unreachable!("String format cannot fail"));
        }

        Ok(query)
    }

    /// Builds the SOSL query string.
    ///
    /// # Panics
    ///
    /// Panics if search text is empty or no objects are specified in RETURNING.
    #[must_use]
    pub fn build(self) -> String {
        self.try_build().unwrap_or_panic("build")
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
fn escape_sosl<'a>(text: impl Into<Cow<'a, str>>) -> Cow<'a, str> {
    let text = text.into();
    let first_special = text.find([
        '?', '&', '|', '!', '{', '}', '[', ']', '(', ')', '^', '~', '*', ':', '\\', '"', '\'', '+',
        '-',
    ]);

    match first_special {
        Some(idx) => {
            let mut escaped = String::with_capacity(text.len() + 8);
            escaped.push_str(&text[..idx]);
            for c in text[idx..].chars() {
                match c {
                    '?' | '&' | '|' | '!' | '{' | '}' | '[' | ']' | '(' | ')' | '^' | '~' | '*'
                    | ':' | '\\' | '"' | '\'' | '+' | '-' => {
                        escaped.push('\\');
                        escaped.push(c);
                    }
                    _ => escaped.push(c),
                }
            }
            Cow::Owned(escaped)
        }
        None => text,
    }
}

/// Validates field syntax to prevent SOSL injection while allowing complex clauses.
///
/// Safe helper for field validation.
fn validate_field_syntax_safe(field: &str) -> Result<(), String> {
    FieldSyntaxValidator::new(field).validate()
}

struct FieldSyntaxValidator<'a> {
    field: &'a str,
    balance: i32,
    in_quote: Option<char>,
    escaped: bool,
}

impl<'a> FieldSyntaxValidator<'a> {
    fn new(field: &'a str) -> Self {
        Self {
            field,
            balance: 0,
            in_quote: None,
            escaped: false,
        }
    }

    fn validate(mut self) -> Result<(), String> {
        for c in self.field.chars() {
            self.process_char(c)?;
        }
        self.verify_completion()
    }

    fn process_char(&mut self, c: char) -> Result<(), String> {
        if self.escaped {
            self.escaped = false;
            return Ok(());
        }

        if c == '\\' {
            self.escaped = true;
            return Ok(());
        }

        if let Some(quote_char) = self.in_quote {
            if c == quote_char {
                self.in_quote = None;
            }
            return Ok(());
        }

        self.process_unquoted_char(c)
    }

    fn process_unquoted_char(&mut self, c: char) -> Result<(), String> {
        match c {
            '\'' | '"' => self.in_quote = Some(c),
            '(' => self.balance += 1,
            ')' => {
                self.balance -= 1;
                if self.balance < 0 {
                    return Err(format!(
                        "unbalanced parentheses (unexpected closing) in field: {}",
                        self.field
                    ));
                }
            }
            _ if c.is_ascii_alphanumeric() => {}
            '_' | '.' | ' ' | '\t' | '\n' | '\r' | ',' | '=' | '!' | '<' | '>' | '-' | '+'
            | ':' | '%' | '&' | '|' | '^' | '*' | '$' => {}
            ';' | '{' | '}' | '[' | ']' => {
                return Err(format!(
                    "field name contains invalid character outside quotes: '{}' in \"{}\"",
                    c, self.field
                ));
            }
            _ => {
                return Err(format!(
                    "field name contains invalid character: '{}' in \"{}\"",
                    c, self.field
                ));
            }
        }
        Ok(())
    }

    fn verify_completion(self) -> Result<(), String> {
        if self.in_quote.is_some() {
            return Err(format!("unclosed quote in field: {}", self.field));
        }
        if self.balance != 0 {
            return Err(format!(
                "unbalanced parentheses (unclosed opening) in field: {}",
                self.field
            ));
        }
        if self.escaped {
            return Err(format!("field cannot end with a backslash: {}", self.field));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::test_utils::must::Must;

    /// Panicking wrapper for `validate_field_syntax_safe` — test-only.
    fn validate_field_syntax(field: &str) {
        if let Err(e) = validate_field_syntax_safe(field) {
            panic!("{e}");
        }
    }

    #[test]
    fn test_returning_invalid_character_fallback() {
        let q = SearchQueryBuilder::new()
            .find("test")
            .returning("Account", &["Invalid@Field"]);
        assert!(q.try_build().is_err());
    }

    #[test]
    #[should_panic(
        expected = "field name contains invalid character outside quotes: ';' in \"Invalid;Field\""
    )]
    fn test_validate_field_syntax_panics() {
        validate_field_syntax("Invalid;Field");
    }

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
    fn test_search_query_builder_sidebar_fields() {
        let query = SearchQueryBuilder::new()
            .find("test@example.com")
            .in_sidebar_fields()
            .returning("Contact", &["Id", "Email"])
            .build();

        assert_eq!(
            query,
            "FIND {test@example.com} IN SIDEBAR FIELDS RETURNING Contact(Id, Email)"
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

    // Test unwrap_or_panic logic by calling panicking methods on invalid states directly.
    #[test]
    #[should_panic(expected = "Invalid input in build: invalid input: search text cannot be empty")]
    fn test_build_panics_on_empty_search_text() {
        let _ = SearchQueryBuilder::new()
            .returning("Account", &["Id"])
            .build();
    }

    #[test]
    #[should_panic(
        expected = "Invalid input in build: invalid input: at least one object must be specified in RETURNING"
    )]
    fn test_build_panics_on_missing_returning() {
        let _ = SearchQueryBuilder::new().find("Test").build();
    }

    #[test]
    fn test_returning_panics_on_invalid_sobject() {
        let q = SearchQueryBuilder::new()
            .find("Test")
            .returning("Invalid;DROP", &["Id"]);
        assert!(q.try_build().is_err());
    }

    #[test]
    fn test_returning_panics_on_invalid_field() {
        let q = SearchQueryBuilder::new()
            .find("Test")
            .returning("Account", &["Invalid;Field"]);
        assert!(q.try_build().is_err());
    }

    #[test]
    #[should_panic(expected = "Invalid input in test_context: invalid input: test error")]
    fn test_unwrap_or_panic_helper() {
        let result: crate::error::Result<()> = Err(crate::error::ForceError::InvalidInput(
            "test error".to_string(),
        ));
        result.unwrap_or_panic("test_context");
    }

    #[test]
    #[should_panic(expected = "Invalid input in test_context: invalid input: test error")]
    fn test_unwrap_or_panic_helper_err() {
        let result: crate::error::Result<()> = Err(crate::error::ForceError::InvalidInput(
            "test error".to_string(),
        ));
        result.unwrap_or_panic("test_context");
    }

    #[test]

    fn test_search_query_builder_try_build_errors() {
        let result = SearchQueryBuilder::new()
            .find("")
            .try_returning("Account", &["Id"])
            .unwrap_or_else(|_| panic!("Valid returning clause failed"))
            .try_build();
        assert!(
            matches!(result, Err(crate::error::ForceError::InvalidInput(msg)) if msg.contains("search text cannot be empty"))
        );

        let result = SearchQueryBuilder::new().find("Test").try_build();
        assert!(
            matches!(result, Err(crate::error::ForceError::InvalidInput(msg)) if msg.contains("at least one object must be specified in RETURNING"))
        );
    }

    #[test]
    fn test_search_query_builder_try_returning_errors() {
        // Invalid sobject
        let result =
            SearchQueryBuilder::new().try_returning("Account; DROP TABLE Account", &["Id"]);
        assert!(
            matches!(result, Err(crate::error::ForceError::InvalidInput(msg)) if msg.contains("SObject name contains invalid characters"))
        );

        // Invalid field syntax outside quotes
        let result = SearchQueryBuilder::new().try_returning("Account", &["Id;"]);
        assert!(
            matches!(result, Err(crate::error::ForceError::InvalidInput(msg)) if msg.contains("invalid character outside quotes"))
        );

        // Invalid field syntax unclosed quotes
        let result = SearchQueryBuilder::new().try_returning("Account", &["Id = 'value"]);
        assert!(
            matches!(result, Err(crate::error::ForceError::InvalidInput(msg)) if msg.contains("unclosed quote"))
        );

        // Invalid field syntax unclosed parenthesis
        let result = SearchQueryBuilder::new().try_returning("Account", &["COUNT(Id"]);
        assert!(
            matches!(result, Err(crate::error::ForceError::InvalidInput(msg)) if msg.contains("unbalanced parentheses (unclosed opening)"))
        );

        // Invalid field syntax unexpected closing parenthesis
        let result = SearchQueryBuilder::new().try_returning("Account", &["COUNT(Id))"]);
        assert!(
            matches!(result, Err(crate::error::ForceError::InvalidInput(msg)) if msg.contains("unbalanced parentheses (unexpected closing)"))
        );
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

    #[test]
    fn test_returning_valid_inputs() {
        let query = SearchQueryBuilder::new()
            .find("test")
            .returning("Account", &["Name", "Id"])
            .returning("Custom__c", &["Field__c", "Parent__r.Name"])
            .build();

        assert_eq!(
            query,
            "FIND {test} RETURNING Account(Name, Id), Custom__c(Field__c, Parent__r.Name)"
        );
    }

    #[test]
    #[should_panic(expected = "SObject name contains invalid characters")]
    fn test_returning_invalid_sobject_space() {
        let _ = SearchQueryBuilder::new()
            .find("test")
            .returning("Account Name", &["Id"])
            .build();
    }

    #[test]
    #[should_panic(expected = "SObject name contains invalid characters")]
    fn test_returning_invalid_sobject_injection() {
        let _ = SearchQueryBuilder::new()
            .find("test")
            .returning("Account; DROP TABLE", &["Id"])
            .build();
    }

    #[test]
    fn test_returning_valid_field_clauses() {
        let query = SearchQueryBuilder::new()
            .find("test")
            .returning("Account", &["Name ORDER BY CreatedDate DESC", "Industry"])
            .build();

        assert_eq!(
            query,
            "FIND {test} RETURNING Account(Name ORDER BY CreatedDate DESC, Industry)"
        );
    }

    #[test]
    #[should_panic(expected = "unbalanced parentheses (unexpected closing) in field")]
    fn test_returning_invalid_field_injection() {
        let _ = SearchQueryBuilder::new()
            .find("test")
            .returning("Account", &["Id) LIMIT 1"])
            .build();
    }

    #[test]
    fn test_returning_valid_function_calls() {
        let query = SearchQueryBuilder::new()
            .find("test")
            .returning(
                "Account",
                &["toLabel(Industry)", "convertCurrency(AnnualRevenue)"],
            )
            .build();

        assert_eq!(
            query,
            "FIND {test} RETURNING Account(toLabel(Industry), convertCurrency(AnnualRevenue))"
        );
    }

    #[test]
    #[should_panic(expected = "unbalanced parentheses (unclosed opening) in field")]
    fn test_returning_invalid_unclosed_parenthesis() {
        let _ = SearchQueryBuilder::new()
            .find("test")
            .returning("Account", &["toLabel(Industry"])
            .build();
    }

    #[test]
    fn test_returning_valid_wildcard_clause() {
        let query = SearchQueryBuilder::new()
            .find("test")
            .returning("Account", &["Name WHERE Name LIKE 'Acme%'"])
            .build();

        assert_eq!(
            query,
            "FIND {test} RETURNING Account(Name WHERE Name LIKE 'Acme%')"
        );
    }

    #[test]
    fn test_returning_valid_complex_clauses() {
        let query = SearchQueryBuilder::new()
            .find("test")
            .returning(
                "Account",
                &["Name WHERE Name = 'Smith & Wesson' AND Industry = 'Tech'"],
            )
            .build();

        assert_eq!(
            query,
            "FIND {test} RETURNING Account(Name WHERE Name = 'Smith & Wesson' AND Industry = 'Tech')"
        );
    }

    #[test]
    fn test_returning_valid_escaped_wildcard() {
        let query = SearchQueryBuilder::new()
            .find("test")
            .returning("Account", &["Name WHERE Name LIKE '100\\%'"])
            .build();

        assert_eq!(
            query,
            "FIND {test} RETURNING Account(Name WHERE Name LIKE '100\\%')"
        );
    }

    #[test]
    fn test_returning_valid_semicolon_inside_quotes() {
        let query = SearchQueryBuilder::new()
            .find("test")
            .returning("Account", &["Name WHERE Name = 'Semi;Colon'"])
            .build();

        assert_eq!(
            query,
            "FIND {test} RETURNING Account(Name WHERE Name = 'Semi;Colon')"
        );
    }

    #[test]
    #[should_panic(expected = "field name contains invalid character outside quotes: ';'")]
    fn test_returning_invalid_semicolon_outside_quotes() {
        let _ = SearchQueryBuilder::new()
            .find("test")
            .returning("Account", &["Name; DROP TABLE"])
            .build();
    }

    #[test]
    fn test_returning_valid_system_variable() {
        let query = SearchQueryBuilder::new()
            .find("test")
            .returning("Account", &["Name WHERE OwnerId = $User.Id"])
            .build();

        assert_eq!(
            query,
            "FIND {test} RETURNING Account(Name WHERE OwnerId = $User.Id)"
        );
    }

    #[test]
    fn test_returning_valid_multiline_query() {
        let query = SearchQueryBuilder::new()
            .find("test")
            .returning("Account", &["Name\nWHERE\tName = 'Acme'"])
            .build();

        assert_eq!(
            query,
            "FIND {test} RETURNING Account(Name\nWHERE\tName = 'Acme')"
        );
    }

    #[test]
    #[should_panic(expected = "field cannot end with a backslash")]
    fn test_returning_invalid_trailing_backslash() {
        let _ = SearchQueryBuilder::new()
            .find("test")
            .returning("Account", &["Name\\"])
            .build();
    }

    #[test]
    fn test_returning_valid_escaped_quote_in_string() {
        let query = SearchQueryBuilder::new()
            .find("test")
            .returning("Account", &["Name WHERE Name = 'O\\'Reilly'"])
            .build();

        assert_eq!(
            query,
            "FIND {test} RETURNING Account(Name WHERE Name = 'O\\'Reilly')"
        );
    }

    #[test]
    fn test_escape_sosl_cow_optimization() {
        // Case 1: No special characters -> Cow::Borrowed
        let safe_text = "SimpleSearch";
        let result = escape_sosl(safe_text);
        assert!(matches!(result, std::borrow::Cow::Borrowed(_)));
        assert_eq!(result, "SimpleSearch");

        // Case 2: Special characters -> Cow::Owned
        let unsafe_text = "Search & Destroy";
        let result = escape_sosl(unsafe_text);
        assert!(matches!(result, std::borrow::Cow::Owned(_)));
        assert_eq!(result, r"Search \& Destroy");

        // Case 3: Owned String input, no escape -> Cow::Owned (reused input)
        let owned_safe = String::from("OwnedString");
        // We pass the string by value (transfer ownership)
        let result_owned = escape_sosl(owned_safe);
        // The implementation checks if special chars exist. If not, it returns the input Cow.
        // Since input was converted to Cow::Owned, output should be Cow::Owned with same data.
        assert!(matches!(result_owned, std::borrow::Cow::Owned(_)));
        assert_eq!(result_owned, "OwnedString");
    }
}

// Integration tests with wiremock
#[cfg(all(test, feature = "mock"))]
mod integration_tests {
    use super::*;
    use crate::client::builder;
    use crate::config::ClientConfig;
    use crate::test_utils::mock_auth::MockAuthenticator;
    use crate::test_utils::must::MustMsg;
    use wiremock::matchers::{bearer_token, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

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
        let Err(err) = result else {
            panic!("Expected an error");
        };
        assert!(err.to_string().contains(""));
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
        let Err(err) = result else {
            panic!("Expected an error");
        };
        assert!(err.to_string().contains(""));
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

        let config = ClientConfig {
            api_version: "v59.0".into(),
            ..Default::default()
        };
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
