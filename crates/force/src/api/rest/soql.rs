//! Safe SOQL query construction.
//!
//! This module provides a builder and utilities for constructing SOQL queries
//! safely, preventing injection vulnerabilities.

use crate::error::ForceError;
use crate::types::validator::{validate_field_name, validate_sobject_name};

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
    let mut escaped = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            '\'' => escaped.push_str(r"\'"),
            '\\' => escaped.push_str(r"\\"),
            '"' => escaped.push_str(r#"\""#),
            _ => escaped.push(c),
        }
    }
    escaped
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
    where_clauses: Vec<String>,
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
        match self.try_select(fields) {
            Ok(builder) => builder,
            Err(e) => panic!("Invalid field name in select: {}", e),
        }
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
        match self.try_from(sobject) {
            Ok(builder) => builder,
            Err(e) => panic!("Invalid SObject name in from: {}", e),
        }
    }

    /// Adds a raw WHERE condition.
    ///
    /// **Warning:** This method does not escape the input. Use with caution.
    #[must_use]
    pub fn where_condition(mut self, condition: impl Into<String>) -> Self {
        self.where_clauses.push(condition.into());
        self
    }

    /// Adds a WHERE condition for equality (e.g., `Field = 'Value'`).
    ///
    /// # Panics
    ///
    /// Panics if the field name is invalid.
    #[must_use]
    pub fn where_eq(mut self, field: &str, value: &str) -> Self {
        if let Err(e) = validate_field_name(field) {
            panic!("Invalid field name in where_eq: {}", e);
        }
        let escaped_value = escape_soql(value);
        self.where_clauses
            .push(format!("{} = '{}'", field, escaped_value));
        self
    }

    /// Adds a WHERE condition for NOT equality (e.g., `Field != 'Value'`).
    ///
    /// # Panics
    ///
    /// Panics if the field name is invalid.
    #[must_use]
    pub fn where_ne(mut self, field: &str, value: &str) -> Self {
        if let Err(e) = validate_field_name(field) {
            panic!("Invalid field name in where_ne: {}", e);
        }
        let escaped_value = escape_soql(value);
        self.where_clauses
            .push(format!("{} != '{}'", field, escaped_value));
        self
    }

    /// Adds a WHERE condition for IN clause (e.g., `Field IN ('Val1', 'Val2')`).
    ///
    /// # Panics
    ///
    /// Panics if the field name is invalid.
    #[must_use]
    pub fn where_in(mut self, field: &str, values: &[impl AsRef<str>]) -> Self {
        if let Err(e) = validate_field_name(field) {
            panic!("Invalid field name in where_in: {}", e);
        }
        if values.is_empty() {
            self.where_clauses.push(format!("{} IN ()", field));
            return self;
        }

        let escaped_values: Vec<String> = values
            .iter()
            .map(|v| format!("'{}'", escape_soql(v.as_ref())))
            .collect();

        self.where_clauses
            .push(format!("{} IN ({})", field, escaped_values.join(", ")));
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
    pub fn where_like(mut self, field: &str, value: &str) -> Self {
        if let Err(e) = validate_field_name(field) {
            panic!("Invalid field name in where_like: {}", e);
        }
        let escaped_value = escape_soql(value);
        self.where_clauses
            .push(format!("{} LIKE '{}'", field, escaped_value));
        self
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
        if let Err(e) = validate_field_name(field) {
            panic!("Invalid field name in order_by: {}", e);
        }
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
        if let Err(e) = validate_field_name(field) {
            panic!("Invalid field name in order_by_desc: {}", e);
        }
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
        let sobject = self.sobject.ok_or_else(|| {
            ForceError::InvalidInput("FROM clause (SObject) is required".to_string())
        })?;

        let mut query = format!("SELECT {} FROM {}", self.fields.join(", "), sobject);

        if !self.where_clauses.is_empty() {
            query.push_str(" WHERE ");
            query.push_str(&self.where_clauses.join(" AND "));
        }

        if let Some(order) = self.order_by {
            query.push_str(" ORDER BY ");
            query.push_str(&order);
        }

        if let Some(limit) = self.limit {
            query.push_str(&format!(" LIMIT {}", limit));
        }

        if let Some(offset) = self.offset {
            query.push_str(&format!(" OFFSET {}", offset));
        }

        Ok(query)
    }

    /// Builds the final SOQL query string (panicking version).
    ///
    /// # Panics
    ///
    /// Panics if no fields are selected or no SObject is specified.
    #[must_use]
    pub fn build(self) -> String {
        match self.try_build() {
            Ok(s) => s,
            Err(e) => panic!("Failed to build SOQL query: {}", e),
        }
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
    fn test_builder_where_in_single() {
        let query = SoqlQueryBuilder::new()
            .select(&["Id"])
            .from("Account")
            .where_in("Name", &["Acme"])
            .build();

        assert_eq!(query, "SELECT Id FROM Account WHERE Name IN ('Acme')");
    }

    #[test]
    fn test_builder_where_in_multiple() {
        let query = SoqlQueryBuilder::new()
            .select(&["Id"])
            .from("Account")
            .where_in("Name", &["Acme", "Globex"])
            .build();

        assert_eq!(
            query,
            "SELECT Id FROM Account WHERE Name IN ('Acme', 'Globex')"
        );
    }

    #[test]
    fn test_builder_where_in_empty() {
        let query = SoqlQueryBuilder::new()
            .select(&["Id"])
            .from("Account")
            .where_in("Name", &[] as &[&str])
            .build();

        // Currently generates IN (). Documenting behavior.
        assert_eq!(query, "SELECT Id FROM Account WHERE Name IN ()");
    }

    #[test]
    fn test_builder_where_like() {
        let query = SoqlQueryBuilder::new()
            .select(&["Id"])
            .from("Account")
            .where_like("Name", "Acme%")
            .build();

        assert_eq!(query, "SELECT Id FROM Account WHERE Name LIKE 'Acme%'");
    }

    #[test]
    fn test_builder_where_like_escaping() {
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
    fn test_builder_limit_offset_order() {
        let query = SoqlQueryBuilder::new()
            .select(&["Id"])
            .from("Account")
            .limit(10)
            .offset(5)
            .order_by("Name")
            .build();

        assert_eq!(
            query,
            "SELECT Id FROM Account ORDER BY Name LIMIT 10 OFFSET 5"
        );
    }

    #[test]
    fn test_builder_order_by_desc() {
        let query = SoqlQueryBuilder::new()
            .select(&["Id"])
            .from("Account")
            .order_by_desc("CreatedDate")
            .build();

        assert_eq!(query, "SELECT Id FROM Account ORDER BY CreatedDate DESC");
    }

    #[test]
    fn test_builder_mixed_conditions() {
        let query = SoqlQueryBuilder::new()
            .select(&["Id"])
            .from("Account")
            .where_eq("Type", "Customer")
            .where_ne("Status", "Inactive")
            .limit(100)
            .build();

        assert_eq!(
            query,
            "SELECT Id FROM Account WHERE Type = 'Customer' AND Status != 'Inactive' LIMIT 100"
        );
    }

    #[test]
    fn test_builder_try_build_errors() {
        // Missing fields
        let builder = SoqlQueryBuilder::new().from("Account");
        assert!(builder.try_build().is_err());

        // Missing SObject
        let builder = SoqlQueryBuilder::new().select(&["Id"]);
        assert!(builder.try_build().is_err());
    }

    #[test]
    #[should_panic(expected = "Invalid field name")]
    fn test_builder_invalid_field_panic() {
        let _ = SoqlQueryBuilder::new().select(&["Name; DROP"]);
    }

    #[test]
    #[should_panic(expected = "Invalid SObject name")]
    fn test_builder_invalid_sobject_panic() {
        let _ = SoqlQueryBuilder::new().from("Account; DROP");
    }
}
