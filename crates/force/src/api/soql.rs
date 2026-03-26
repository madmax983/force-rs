//! Safe SOQL query construction.
//!
//! This module provides a builder and utilities for constructing SOQL queries
//! safely, preventing injection vulnerabilities.

use crate::error::ForceError;
use crate::types::validator::{validate_field_name, validate_sobject_name};
use std::borrow::Cow;

/// Escapes special characters for SOQL string literals.
///
/// According to Salesforce documentation, the following characters must be escaped:
/// - Single quote (`'`) -> `\'`
/// - Backslash (`\`) -> `\\`
/// - Double quote (`"`) -> `\"` (if using double quotes)
///
/// # Examples
///
/// ```
/// use force::api::soql::escape_soql;
///
/// assert_eq!(escape_soql("O'Reilly"), r"O\'Reilly");
/// assert_eq!(escape_soql(r"C:\Docs"), r"C:\\Docs");
/// ```
#[must_use]
pub fn escape_soql(input: &str) -> String {
    escape_soql_cow(input).into_owned()
}

/// Zero-cost abstraction for SOQL escaping.
///
/// Returns `Cow::Borrowed` if no escaping is required, avoiding allocation.
/// Returns `Cow::Owned` if escaping is needed.
pub fn escape_soql_cow(input: &str) -> Cow<'_, str> {
    let first_special = input.find(['\'', '\\', '"']);

    match first_special {
        Some(idx) => {
            let mut escaped = String::with_capacity(input.len() + 8);
            escaped.push_str(&input[..idx]);

            // Note: `input.find()` returns a byte index.
            // Using `input[idx..]` here is technically safe because we know the needle
            // `['\'', '\\', '"']` are 1-byte ASCII characters, meaning `idx` will
            // always align with a char boundary for `input`. If we searched for a
            // multi-byte char this would panic.
            for c in input[idx..].chars() {
                match c {
                    '\'' => escaped.push_str(r"\'"),
                    '\\' => escaped.push_str(r"\\"),
                    '"' => escaped.push_str(r#"\""#),
                    _ => escaped.push(c),
                }
            }
            Cow::Owned(escaped)
        }
        None => Cow::Borrowed(input),
    }
}

/// Encodes a `SoqlQueryBuilder` into a URL-safe `query?q=...` string.
///
/// Validates the builder, then writes the SOQL through URL-encoding.
/// Used by Composite Batch and Graph APIs to embed queries in subrequests.
#[cfg(any(feature = "composite", feature = "composite_graph"))]
pub(crate) fn encode_soql_query_url(
    query_builder: &SoqlQueryBuilder,
) -> Result<String, ForceError> {
    if let Err(e) = query_builder.validate() {
        return Err(ForceError::InvalidInput(format!(
            "Invalid query builder: {e}"
        )));
    }

    let mut url = String::with_capacity(256 + 8);
    url.push_str("query?q=");

    {
        let mut writer = crate::api::url_encoded_writer::UrlEncodedWriter(&mut url);
        query_builder
            .write_query(&mut writer)
            .map_err(|e| ForceError::InvalidInput(format!("Formatting failed: {e}")))?;
    }

    Ok(url)
}

/// Builder for constructing safe SOQL queries.
///
/// Helps prevent SOQL injection by validating object and field names,
/// and automatically escaping string literals in WHERE clauses.
///
/// # Examples
///
/// ```
/// use force::api::soql::SoqlQueryBuilder;
///
/// let query = SoqlQueryBuilder::new()
///     .select(&["Id", "Name"])
///     .from("Account")
///     .where_eq("Name", "Acme Corp")
///     .limit(10)
///     .build();
///
/// assert_eq!(query, "SELECT Id, Name FROM Account WHERE Name = 'Acme Corp' LIMIT 10");
/// ```
#[derive(Debug, Default, Clone)]
pub struct SoqlQueryBuilder {
    fields: Vec<String>,
    sobject: Option<String>,
    where_clauses: Vec<String>,
    limit: Option<u32>,
    offset: Option<u32>,
    order_by: Option<String>,
    error: Option<String>,
}

impl SoqlQueryBuilder {
    /// Creates a new empty SOQL query builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the fields to select.
    ///
    /// # Errors
    ///
    /// Returns an error if any field name contains invalid characters.
    #[must_use]
    pub fn select(mut self, fields: &[impl AsRef<str>]) -> Self {
        for f in fields {
            if let Err(e) = validate_field_name(f.as_ref()) {
                self.error = Some(e.to_string());
                return self;
            }
        }
        self.fields = fields.iter().map(|f| f.as_ref().to_string()).collect();
        self
    }

    /// Sets the SObject to select from.
    ///
    /// # Errors
    ///
    /// Returns an error if the SObject name is invalid.
    #[must_use]
    pub fn from(mut self, sobject: impl Into<String>) -> Self {
        let s = sobject.into();
        if let Err(e) = validate_sobject_name(&s) {
            self.error = Some(e.to_string());
            return self;
        }
        self.sobject = Some(s);
        self
    }

    /// Adds a raw WHERE condition without escaping.
    ///
    /// **Warning:** This method does not escape the input. It is susceptible to SOQL injection
    /// if used with untrusted user input. Prefer using parameterized/escaped alternatives
    /// like `where_eq` when possible.
    ///
    /// # Examples
    ///
    /// ```
    /// use force::api::soql::SoqlQueryBuilder;
    /// let query = SoqlQueryBuilder::new()
    ///     .select(&["Id"])
    ///     .from("Account")
    ///     .where_condition_unchecked("CreatedDate > LAST_N_DAYS:30")
    ///     .build();
    /// assert_eq!(query, "SELECT Id FROM Account WHERE CreatedDate > LAST_N_DAYS:30");
    /// ```
    #[must_use]
    pub fn where_condition_unchecked(mut self, condition: impl Into<String>) -> Self {
        self.where_clauses.push(condition.into());
        self
    }

    /// Adds a WHERE condition for equality (e.g., `Field = 'Value'`).
    ///
    /// # Panics
    ///
    /// Panics if the field name is invalid.
    ///
    /// # Examples
    ///
    /// ```
    /// use force::api::soql::SoqlQueryBuilder;
    /// let query = SoqlQueryBuilder::new()
    ///     .select(&["Id"])
    ///     .from("Contact")
    ///     .where_eq("LastName", "Smith")
    ///     .build();
    /// assert_eq!(query, "SELECT Id FROM Contact WHERE LastName = 'Smith'");
    /// ```
    #[must_use]
    pub fn where_eq(self, field: &str, value: &str) -> Self {
        self.add_condition(field, "=", value)
    }

    /// Adds a WHERE condition for NOT equality (e.g., `Field != 'Value'`).
    ///
    /// # Panics
    ///
    /// Panics if the field name is invalid.
    ///
    /// # Examples
    ///
    /// ```
    /// use force::api::soql::SoqlQueryBuilder;
    /// let query = SoqlQueryBuilder::new()
    ///     .select(&["Id"])
    ///     .from("Contact")
    ///     .where_ne("LastName", "Smith")
    ///     .build();
    /// assert_eq!(query, "SELECT Id FROM Contact WHERE LastName != 'Smith'");
    /// ```
    #[must_use]
    pub fn where_ne(self, field: &str, value: &str) -> Self {
        self.add_condition(field, "!=", value)
    }

    /// Adds a simple WHERE condition (helper).
    fn add_condition(mut self, field: &str, op: &str, value: &str) -> Self {
        use std::fmt::Write;

        if let Err(e) = validate_field_name(field) {
            self.error = Some(e.to_string());
            return self;
        }

        let escaped_value = escape_soql_cow(value);
        let capacity = field.len() + op.len() + escaped_value.len() + 4;
        let mut buffer = String::with_capacity(capacity);
        write!(buffer, "{} {} '{}'", field, op, escaped_value)
            .unwrap_or_else(|_| unreachable!("writing to String is infallible"));

        self.where_clauses.push(buffer);
        self
    }

    /// Adds a WHERE condition for IN clause (e.g., `Field IN ('Val1', 'Val2')`).
    ///
    /// # Panics
    ///
    /// Panics if the field name is invalid.
    ///
    /// # Examples
    ///
    /// ```
    /// use force::api::soql::SoqlQueryBuilder;
    /// let query = SoqlQueryBuilder::new()
    ///     .select(&["Id"])
    ///     .from("Account")
    ///     .where_in("Industry", &["Technology", "Finance"])
    ///     .build();
    /// assert_eq!(query, "SELECT Id FROM Account WHERE Industry IN ('Technology', 'Finance')");
    /// ```
    ///
    /// **Performance:** Avoids intermediate `Vec<String>` heap allocations by pre-calculating capacity
    /// and writing the escaped SOQL string directly into a single formatted string buffer.
    #[must_use]
    pub fn where_in(mut self, field: &str, values: &[impl AsRef<str>]) -> Self {
        use std::fmt::Write;

        if let Err(e) = validate_field_name(field) {
            self.error = Some(e.to_string());
            return self;
        }

        if values.is_empty() {
            let mut buffer = String::with_capacity(field.len() + 7);
            buffer.push_str(field);
            buffer.push_str(" IN ()");
            self.where_clauses.push(buffer);
            return self;
        }

        let capacity = field.len() + 6 + (values.len() * 14);
        let mut buffer = String::with_capacity(capacity);
        buffer.push_str(field);
        buffer.push_str(" IN (");

        for (i, value) in values.iter().enumerate() {
            if i > 0 {
                buffer.push_str(", ");
            }
            let escaped = escape_soql_cow(value.as_ref());
            write!(buffer, "'{}'", escaped)
                .unwrap_or_else(|_| unreachable!("writing to String is infallible"));
        }
        buffer.push(')');

        self.where_clauses.push(buffer);
        self
    }

    /// Adds a WHERE condition for LIKE clause (e.g., `Field LIKE 'Val%'`).
    ///
    /// **Note:** Preserves wildcards (`%`, `_`) but escapes quotes/backslashes.
    ///
    /// # Panics
    ///
    /// Panics if the field name is invalid.
    ///
    /// # Examples
    ///
    /// ```
    /// use force::api::soql::SoqlQueryBuilder;
    /// let query = SoqlQueryBuilder::new()
    ///     .select(&["Id"])
    ///     .from("Account")
    ///     .where_like("Name", "Acme%")
    ///     .build();
    /// assert_eq!(query, "SELECT Id FROM Account WHERE Name LIKE 'Acme%'");
    /// ```
    #[must_use]
    pub fn where_like(self, field: &str, value: &str) -> Self {
        self.add_condition(field, "LIKE", value)
    }

    /// Sets the LIMIT clause.
    ///
    /// # Examples
    ///
    /// ```
    /// use force::api::soql::SoqlQueryBuilder;
    /// let query = SoqlQueryBuilder::new()
    ///     .select(&["Id"])
    ///     .from("Account")
    ///     .limit(5)
    ///     .build();
    /// assert_eq!(query, "SELECT Id FROM Account LIMIT 5");
    /// ```
    #[must_use]
    pub fn limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Sets the OFFSET clause.
    ///
    /// # Examples
    ///
    /// ```
    /// use force::api::soql::SoqlQueryBuilder;
    /// let query = SoqlQueryBuilder::new()
    ///     .select(&["Id"])
    ///     .from("Account")
    ///     .limit(10)
    ///     .offset(20)
    ///     .build();
    /// assert_eq!(query, "SELECT Id FROM Account LIMIT 10 OFFSET 20");
    /// ```
    #[must_use]
    pub fn offset(mut self, offset: u32) -> Self {
        self.offset = Some(offset);
        self
    }

    /// Sets the ORDER BY clause.
    ///
    /// # Panics
    ///
    /// Panics if the field name is invalid.
    ///
    /// # Examples
    ///
    /// ```
    /// use force::api::soql::SoqlQueryBuilder;
    /// let query = SoqlQueryBuilder::new()
    ///     .select(&["Id"])
    ///     .from("Account")
    ///     .order_by("Name")
    ///     .build();
    /// assert_eq!(query, "SELECT Id FROM Account ORDER BY Name");
    /// ```
    #[must_use]
    pub fn order_by(mut self, field: &str) -> Self {
        if let Err(e) = validate_field_name(field) {
            self.error = Some(e.to_string());
            return self;
        }
        self.order_by = Some(field.to_string());
        self
    }

    /// Sets the ORDER BY clause with direction (DESC).
    ///
    /// # Panics
    ///
    /// Panics if the field name is invalid.
    ///
    /// # Examples
    ///
    /// ```
    /// use force::api::soql::SoqlQueryBuilder;
    /// let query = SoqlQueryBuilder::new()
    ///     .select(&["Id"])
    ///     .from("Account")
    ///     .order_by_desc("CreatedDate")
    ///     .build();
    /// assert_eq!(query, "SELECT Id FROM Account ORDER BY CreatedDate DESC");
    /// ```
    #[must_use]
    pub fn order_by_desc(mut self, field: &str) -> Self {
        if let Err(e) = validate_field_name(field) {
            self.error = Some(e.to_string());
            return self;
        }
        self.order_by = Some(format!("{} DESC", field));
        self
    }

    /// Validates that the builder has all necessary components to build a query.
    ///
    /// # Errors
    ///
    /// Returns an error if no fields are selected or no SObject is specified.
    pub fn validate(&self) -> Result<(), ForceError> {
        if let Some(err) = &self.error {
            return Err(ForceError::InvalidInput(err.clone()));
        }
        if let Some(err) = &self.error {
            return Err(ForceError::InvalidInput(err.clone()));
        }
        if self.fields.is_empty() {
            return Err(ForceError::InvalidInput(
                "Select fields cannot be empty".to_string(),
            ));
        }
        if self.sobject.is_none() {
            return Err(ForceError::InvalidInput(
                "FROM clause (SObject) is required".to_string(),
            ));
        }
        Ok(())
    }

    /// Writes the SOQL query to the provided writer.
    ///
    /// This method allows streaming the query directly to a buffer or serializer
    /// without allocating an intermediate String.
    ///
    /// # Errors
    ///
    /// Returns `fmt::Error` if writing to the underlying writer fails.
    /// Note: Validation errors are NOT checked here; call `validate()` first.
    pub(crate) fn write_query<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        self.append_fields(w)?;

        w.write_str(" FROM ")?;
        if let Some(sobject) = &self.sobject {
            w.write_str(sobject)?;
        }

        self.append_where_clauses(w)?;
        self.append_modifiers(w)?;
        Ok(())
    }

    /// Builds the final SOQL query string.
    ///
    /// # Errors
    ///
    /// Returns an error if no fields are selected or no SObject is specified.
    pub fn try_build(self) -> Result<String, ForceError> {
        self.validate()?;

        // 256 is a reasonable default to avoid immediate reallocations
        // without complex pre-calculation logic (YAGNI).
        let mut query = String::with_capacity(256);

        self.write_query(&mut query)
            .map_err(|_| ForceError::InvalidInput("Formatting error".to_string()))?;

        Ok(query)
    }

    fn append_fields<W: std::fmt::Write>(&self, query: &mut W) -> std::fmt::Result {
        query.write_str("SELECT ")?;
        for (i, field) in self.fields.iter().enumerate() {
            if i > 0 {
                query.write_str(", ")?;
            }
            query.write_str(field)?;
        }
        Ok(())
    }

    fn append_where_clauses<W: std::fmt::Write>(&self, query: &mut W) -> std::fmt::Result {
        if self.where_clauses.is_empty() {
            return Ok(());
        }

        query.write_str(" WHERE ")?;
        for (i, clause) in self.where_clauses.iter().enumerate() {
            if i > 0 {
                query.write_str(" AND ")?;
            }
            query.write_str(clause)?;
        }
        Ok(())
    }

    fn append_modifiers<W: std::fmt::Write>(&self, query: &mut W) -> std::fmt::Result {
        if let Some(order) = &self.order_by {
            query.write_str(" ORDER BY ")?;
            query.write_str(order)?;
        }

        if let Some(limit) = self.limit {
            write!(query, " LIMIT {}", limit)?;
        }

        if let Some(offset) = self.offset {
            write!(query, " OFFSET {}", offset)?;
        }
        Ok(())
    }

    fn unwrap_or_panic<T>(result: Result<T, ForceError>, context: &str) -> T {
        result.unwrap_or_else(|e| panic!("Invalid input in {}: {}", context, e))
    }

    /// Builds the final SOQL query string (panicking version).
    ///
    /// # Panics
    ///
    /// Panics if no fields are selected or no SObject is specified.
    #[must_use]
    pub fn build(self) -> String {
        match self.try_build() {
            Ok(query) => query,
            Err(e) => panic!("Invalid input in build: {}", e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Must;

    #[test]
    fn test_escape_soql() {
        assert_eq!(escape_soql("Normal String"), "Normal String");
        assert_eq!(escape_soql("O'Reilly"), r"O\'Reilly");
        assert_eq!(escape_soql(r"C:\Path"), r"C:\\Path");
        assert_eq!(escape_soql(r#"Quote " in text"#), r#"Quote \" in text"#);
    }

    #[test]
    fn test_builder_basic() {
        let query = SoqlQueryBuilder::new()
            .select(&["Id", "Name"])
            .from("Account")
            .build();

        assert_eq!(query, "SELECT Id, Name FROM Account");
    }

    #[test]
    fn test_builder_where_eq() {
        let query = SoqlQueryBuilder::new()
            .select(&["Id"])
            .from("Contact")
            .where_eq("LastName", "O'Connor")
            .build();

        assert_eq!(
            query,
            "SELECT Id FROM Contact WHERE LastName = 'O\\'Connor'"
        );
    }

    #[test]
    fn test_builder_where_ne() {
        let query = SoqlQueryBuilder::new()
            .select(&["Id"])
            .from("Contact")
            .where_ne("LastName", "O'Connor")
            .build();

        assert_eq!(
            query,
            "SELECT Id FROM Contact WHERE LastName != 'O\\'Connor'"
        );
    }

    #[test]
    fn test_where_condition_raw() {
        let query = SoqlQueryBuilder::new()
            .select(&["Id", "Amount"])
            .from("Opportunity")
            .where_condition_unchecked("Amount > 1000")
            .build();

        assert_eq!(
            query,
            "SELECT Id, Amount FROM Opportunity WHERE Amount > 1000"
        );
    }

    #[test]
    fn test_validate_sobject_name() {
        assert!(validate_sobject_name("Account").is_ok());
        assert!(validate_sobject_name("Custom__c").is_ok());
        assert!(validate_sobject_name("Account; DROP").is_err());
        assert!(validate_sobject_name("Account Name").is_err()); // No spaces
    }

    #[test]
    fn test_validate_field_name() {
        assert!(validate_field_name("Name").is_ok());
        assert!(validate_field_name("Custom__c").is_ok());
        assert!(validate_field_name("Parent.Name").is_ok());
        assert!(validate_field_name("count(Id)").is_ok());
        assert!(validate_field_name("toLabel(StageName)").is_ok());

        assert!(validate_field_name("Name; DROP").is_err());
        assert!(validate_field_name("Name--").is_err());
        assert!(validate_field_name("count(Id").is_err()); // Unbalanced
    }

    #[test]
    fn test_escape_soql_cow_optimization() {
        // Case 1: No escape needed -> should be Borrowed
        let safe = "SafeString123";
        let result = escape_soql_cow(safe);
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "SafeString123");

        // Case 2: Escape needed -> should be Owned
        let unsafe_str = "O'Reilly";
        let result = escape_soql_cow(unsafe_str);
        assert!(matches!(result, Cow::Owned(_)));
        assert_eq!(result, r"O\'Reilly");
    }

    #[test]
    fn test_optimized_query_construction() {
        let query = SoqlQueryBuilder::new()
            .select(&["Id", "Name", "BillingCity"])
            .from("Account")
            .where_eq("Type", "Customer")
            .where_in("Industry", &["Tech", "Finance"])
            .order_by_desc("CreatedDate")
            .limit(100)
            .offset(50)
            .build();

        let expected = "SELECT Id, Name, BillingCity FROM Account WHERE Type = 'Customer' AND Industry IN ('Tech', 'Finance') ORDER BY CreatedDate DESC LIMIT 100 OFFSET 50";
        assert_eq!(query, expected);
    }

    #[test]
    fn test_builder_try_methods_errors() {
        let builder = SoqlQueryBuilder::new();

        // Invalid field name
        let builder_err = builder.clone().select(&["Valid", "Invalid;DROP"]);
        let result = builder_err.try_build();

        if let Err(ForceError::InvalidInput(msg)) = result {
            assert!(msg.contains("invalid character"));
        } else {
            panic!("Expected ForceError::InvalidInput");
        }

        // Invalid SObject name
        let builder_err = builder.from("Invalid SObject");
        let result = builder_err.try_build();

        if let Err(ForceError::InvalidInput(msg)) = result {
            assert!(msg.contains("invalid characters"));
        } else {
            panic!("Expected ForceError::InvalidInput");
        }
    }

    #[test]
    #[should_panic(
        expected = "Invalid input in build: invalid input: Select fields cannot be empty"
    )]
    fn test_build_panics_on_missing_fields() {
        let _ = SoqlQueryBuilder::new().from("Account").build();
    }

    #[test]
    #[should_panic(
        expected = "Invalid input in build: invalid input: FROM clause (SObject) is required"
    )]
    fn test_build_panics_on_missing_sobject() {
        let _ = SoqlQueryBuilder::new().select(&["Id"]).build();
    }

    #[test]
    #[should_panic(expected = "Invalid input in test_context: invalid input: test error")]
    fn test_unwrap_or_panic_helper() {
        SoqlQueryBuilder::unwrap_or_panic::<()>(
            Err(ForceError::InvalidInput("test error".to_string())),
            "test_context",
        );
    }

    #[test]

    fn test_build_errors() {
        // Missing fields
        let builder = SoqlQueryBuilder::new().from("Account");
        let result = builder.try_build();
        match result {
            Err(e) => assert_eq!(
                e.to_string(),
                "invalid input: Select fields cannot be empty"
            ),
            Ok(_) => panic!("Expected error"),
        }

        // Missing SObject
        let builder = SoqlQueryBuilder::new().select(&["Id"]);
        let result = builder.try_build();
        match result {
            Err(e) => assert_eq!(
                e.to_string(),
                "invalid input: FROM clause (SObject) is required"
            ),
            Ok(_) => panic!("Expected error"),
        }
    }

    #[test]
    fn test_where_in_edge_cases() {
        // Empty list
        let query = SoqlQueryBuilder::new()
            .select(&["Id"])
            .from("Account")
            .where_in("Name", &[] as &[&str])
            .build();
        assert_eq!(query, "SELECT Id FROM Account WHERE Name IN ()");

        // Single item
        let query = SoqlQueryBuilder::new()
            .select(&["Id"])
            .from("Account")
            .where_in("Name", &["One"])
            .build();
        assert_eq!(query, "SELECT Id FROM Account WHERE Name IN ('One')");
    }

    #[test]
    fn test_where_like() {
        let query = SoqlQueryBuilder::new()
            .select(&["Id"])
            .from("Account")
            .where_like("Name", "Acme%")
            .build();
        assert_eq!(query, "SELECT Id FROM Account WHERE Name LIKE 'Acme%'");

        // Escaping in LIKE
        let query = SoqlQueryBuilder::new()
            .select(&["Id"])
            .from("Account")
            .where_like("Name", "O'Reilly%")
            .build();
        assert_eq!(
            query,
            "SELECT Id FROM Account WHERE Name LIKE 'O\\'Reilly%'"
        );
    }

    #[test]
    fn test_limit_offset_only() {
        let query = SoqlQueryBuilder::new()
            .select(&["Id"])
            .from("Account")
            .limit(10)
            .offset(5)
            .build();
        assert_eq!(query, "SELECT Id FROM Account LIMIT 10 OFFSET 5");
    }

    #[test]
    fn test_order_independence() {
        // Build in random order
        let query = SoqlQueryBuilder::new()
            .limit(10)
            .where_eq("Name", "Acme")
            .select(&["Id"])
            .offset(5)
            .from("Account")
            .build();

        // Output should be standard SOQL order: SELECT ... FROM ... WHERE ... LIMIT ... OFFSET
        assert_eq!(
            query,
            "SELECT Id FROM Account WHERE Name = 'Acme' LIMIT 10 OFFSET 5"
        );
    }

    #[test]
    fn test_write_query_streaming() {
        let builder = SoqlQueryBuilder::new().select(&["Id"]).from("Account");

        builder.validate().must();

        let mut buffer = String::new();
        builder.write_query(&mut buffer).must();

        assert_eq!(buffer, "SELECT Id FROM Account");
    }
}
