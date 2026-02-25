//! Safe SOQL query construction.
//!
//! This module provides a builder and utilities for constructing SOQL queries
//! safely, preventing injection vulnerabilities.

use crate::error::ForceError;
use crate::types::validator::{validate_field_name, validate_sobject_name};
use std::borrow::Cow;
use std::fmt::Write;

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
/// use force::api::rest::soql::escape_soql;
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
pub(crate) fn escape_soql_cow(input: &str) -> Cow<'_, str> {
    let first_special = input.find(['\'', '\\', '"']);

    match first_special {
        Some(idx) => {
            let mut escaped = String::with_capacity(input.len() + 8);
            escaped.push_str(&input[..idx]);

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

#[derive(Debug, Clone)]
enum WhereClause {
    Raw(String),
    In { field: String, values: Vec<String> },
}

impl std::fmt::Display for WhereClause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Raw(s) => write!(f, "{}", s),
            Self::In { field, values } => {
                write!(f, "{} IN (", field)?;
                for (j, v) in values.iter().enumerate() {
                    if j > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "'{}'", v)?;
                }
                write!(f, ")")
            }
        }
    }
}

/// Builder for constructing safe SOQL queries.
///
/// Helps prevent SOQL injection by validating object and field names,
/// and automatically escaping string literals in WHERE clauses.
///
/// # Examples
///
/// ```
/// use force::api::rest::SoqlQueryBuilder;
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
    where_clauses: Vec<WhereClause>,
    limit: Option<u32>,
    offset: Option<u32>,
    order_by: Option<String>,
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
    pub fn try_select(mut self, fields: &[impl AsRef<str>]) -> Result<Self, ForceError> {
        self.fields = fields
            .iter()
            .map(|f| {
                let s = f.as_ref();
                validate_field_name(s)?;
                Ok(s.to_string())
            })
            .collect::<Result<Vec<_>, ForceError>>()?;
        Ok(self)
    }

    /// Sets the fields to select (panicking version).
    ///
    /// # Panics
    ///
    /// Panics if any field name contains invalid characters.
    #[must_use]
    pub fn select(self, fields: &[impl AsRef<str>]) -> Self {
        Self::unwrap_or_panic(self.try_select(fields), "select")
    }

    /// Sets the SObject to select from.
    ///
    /// # Errors
    ///
    /// Returns an error if the SObject name is invalid.
    pub fn try_from(mut self, sobject: impl Into<String>) -> Result<Self, ForceError> {
        let s = sobject.into();
        validate_sobject_name(&s)?;
        self.sobject = Some(s);
        Ok(self)
    }

    /// Sets the SObject to select from (panicking version).
    ///
    /// # Panics
    ///
    /// Panics if the SObject name contains invalid characters.
    #[must_use]
    pub fn from(self, sobject: impl Into<String>) -> Self {
        Self::unwrap_or_panic(self.try_from(sobject), "from")
    }

    /// Adds a raw WHERE condition.
    ///
    /// **Warning:** This method does not escape the input. Use with caution.
    #[must_use]
    pub fn where_condition(mut self, condition: impl Into<String>) -> Self {
        self.where_clauses.push(WhereClause::Raw(condition.into()));
        self
    }

    /// Adds a WHERE condition for equality (e.g., `Field = 'Value'`).
    ///
    /// # Panics
    ///
    /// Panics if the field name is invalid.
    #[must_use]
    pub fn where_eq(self, field: &str, value: &str) -> Self {
        self.add_condition(field, "=", value, "where_eq")
    }

    /// Adds a WHERE condition for NOT equality (e.g., `Field != 'Value'`).
    ///
    /// # Panics
    ///
    /// Panics if the field name is invalid.
    #[must_use]
    pub fn where_ne(self, field: &str, value: &str) -> Self {
        self.add_condition(field, "!=", value, "where_ne")
    }

    /// Adds a simple WHERE condition (helper).
    fn add_condition(mut self, field: &str, op: &str, value: &str, context: &str) -> Self {
        Self::validate_field(field, context);
        // Optimization: Use escape_soql_cow to avoid allocation if escape not needed
        let escaped_value = escape_soql_cow(value);
        self.where_clauses.push(WhereClause::Raw(format!(
            "{} {} '{}'",
            field, op, escaped_value
        )));
        self
    }

    /// Helper to validate field names and panic on error.
    fn validate_field(field: &str, context: &str) {
        if let Err(e) = validate_field_name(field) {
            panic!("Invalid field name in {}: {}", context, e);
        }
    }

    /// Adds a WHERE condition for IN clause (e.g., `Field IN ('Val1', 'Val2')`).
    ///
    /// # Panics
    ///
    /// Panics if the field name is invalid.
    #[must_use]
    pub fn where_in(mut self, field: &str, values: &[impl AsRef<str>]) -> Self {
        Self::validate_field(field, "where_in");
        if values.is_empty() {
            self.where_clauses
                .push(WhereClause::Raw(format!("{} IN ()", field)));
            return self;
        }

        let escaped_values: Vec<String> = values.iter().map(|v| escape_soql(v.as_ref())).collect();

        self.where_clauses.push(WhereClause::In {
            field: field.to_string(),
            values: escaped_values,
        });
        self
    }

    /// Adds a WHERE condition for LIKE clause (e.g., `Field LIKE 'Val%'`).
    ///
    /// **Note:** Preserves wildcards (`%`, `_`) but escapes quotes/backslashes.
    ///
    /// # Panics
    ///
    /// Panics if the field name is invalid.
    #[must_use]
    pub fn where_like(self, field: &str, value: &str) -> Self {
        self.add_condition(field, "LIKE", value, "where_like")
    }

    /// Sets the LIMIT clause.
    #[must_use]
    pub fn limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Sets the OFFSET clause.
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
    #[must_use]
    pub fn order_by(mut self, field: &str) -> Self {
        Self::validate_field(field, "order_by");
        self.order_by = Some(field.to_string());
        self
    }

    /// Sets the ORDER BY clause with direction (DESC).
    ///
    /// # Panics
    ///
    /// Panics if the field name is invalid.
    #[must_use]
    pub fn order_by_desc(mut self, field: &str) -> Self {
        Self::validate_field(field, "order_by_desc");
        self.order_by = Some(format!("{} DESC", field));
        self
    }

    /// Builds the final SOQL query string.
    ///
    /// # Errors
    ///
    /// Returns an error if no fields are selected or no SObject is specified.
    pub fn try_build(self) -> Result<String, ForceError> {
        if self.fields.is_empty() {
            return Err(ForceError::InvalidInput(
                "Select fields cannot be empty".to_string(),
            ));
        }
        let sobject = self.sobject.as_ref().ok_or_else(|| {
            ForceError::InvalidInput("FROM clause (SObject) is required".to_string())
        })?;

        // 256 is a reasonable default to avoid immediate reallocations
        // without complex pre-calculation logic (YAGNI).
        let mut query = String::with_capacity(256);

        self.append_fields(&mut query);

        query.push_str(" FROM ");
        query.push_str(sobject);

        self.append_where_clauses(&mut query);
        self.append_modifiers(&mut query);

        Ok(query)
    }

    fn append_fields(&self, query: &mut String) {
        query.push_str("SELECT ");
        for (i, field) in self.fields.iter().enumerate() {
            if i > 0 {
                query.push_str(", ");
            }
            query.push_str(field);
        }
    }

    fn append_where_clauses(&self, query: &mut String) {
        if self.where_clauses.is_empty() {
            return;
        }

        query.push_str(" WHERE ");
        for (i, clause) in self.where_clauses.iter().enumerate() {
            if i > 0 {
                query.push_str(" AND ");
            }
            let _ = write!(query, "{}", clause);
        }
    }

    fn append_modifiers(&self, query: &mut String) {
        if let Some(order) = &self.order_by {
            query.push_str(" ORDER BY ");
            query.push_str(order);
        }

        if let Some(limit) = self.limit {
            let _ = write!(query, " LIMIT {}", limit);
        }

        if let Some(offset) = self.offset {
            let _ = write!(query, " OFFSET {}", offset);
        }
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
        Self::unwrap_or_panic(self.try_build(), "build")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let result = builder.clone().try_select(&["Valid", "Invalid;DROP"]);
        assert!(result.is_err());
        if let Err(ForceError::InvalidInput(msg)) = result {
            assert!(msg.contains("invalid character"));
        } else {
            panic!("Expected ForceError::InvalidInput");
        }

        // Invalid SObject name
        let result = builder.try_from("Invalid SObject");
        assert!(result.is_err());
        if let Err(ForceError::InvalidInput(msg)) = result {
            assert!(msg.contains("invalid characters"));
        } else {
            panic!("Expected ForceError::InvalidInput");
        }
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
}
