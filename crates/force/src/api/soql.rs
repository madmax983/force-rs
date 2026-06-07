//! Safe SOQL query construction.
//!
//! This module provides a builder and utilities for constructing SOQL queries
//! safely, preventing injection vulnerabilities.

use crate::api::builder_unwrap::BuilderUnwrapExt;
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
/// use force::api::escape_soql;
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
pub fn encode_soql_query_url(query_builder: &SoqlQueryBuilder) -> crate::error::Result<String> {
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
/// use force::api::SoqlQueryBuilder;
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

    /// Creates a new SOQL query builder initialized with all queryable fields from an `SObjectDescribe`.
    ///
    /// This essentially performs a safe `SELECT *` for the given SObject, ensuring only fields
    /// that the user has permission to query are included.
    #[must_use]
    pub fn from_describe(describe: &crate::types::describe::SObjectDescribe) -> Self {
        let mut fields = Vec::with_capacity(describe.fields.len());
        for field in &describe.fields {
            // Note: In some old API versions or specific objects, a field might not be explicitly
            // marked `queryable` but can still be queried if we have read access. However, relying
            // on the metadata API directly is the safest default. We filter out deprecated/hidden fields.
            // SObjectDescribe's FieldDescribe doesn't have a `queryable` boolean directly on it
            // but we can infer queryability. Usually fields that are deprecated/hidden are not queryable.
            if !field.deprecated_and_hidden {
                fields.push(field.name.clone());
            }
        }

        Self {
            fields,
            sobject: Some(describe.name.clone()),
            ..Default::default()
        }
    }

    /// Sets the fields to select.
    ///
    /// # Errors
    ///
    /// Returns an error if any field name contains invalid characters.
    pub fn try_select(mut self, fields: &[impl AsRef<str>]) -> crate::error::Result<Self> {
        #[allow(unused_doc_comments)]
        /// ⚡ Bolt: Pre-allocating capacity avoids multiple heap reallocations
        /// that would occur when using `.collect::<Result<Vec<_>, _>>()`
        let mut new_fields = Vec::with_capacity(fields.len());
        for f in fields {
            let s = f.as_ref();
            validate_field_name(s)?;
            new_fields.push(s.to_string());
        }
        self.fields = new_fields;
        Ok(self)
    }

    /// Sets the fields to select (panicking version).
    ///
    /// # Panics
    ///
    /// Panics if any field name contains invalid characters.
    #[must_use]
    pub fn select(self, fields: &[impl AsRef<str>]) -> Self {
        self.try_select(fields).unwrap_or_panic("select")
    }

    /// Sets the SObject to select from.
    ///
    /// # Errors
    ///
    /// Returns an error if the SObject name is invalid.
    pub fn try_from(mut self, sobject: impl Into<String>) -> crate::error::Result<Self> {
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
        self.try_from(sobject).unwrap_or_panic("from")
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
    /// use force::api::SoqlQueryBuilder;
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
    /// use force::api::SoqlQueryBuilder;
    /// let query = SoqlQueryBuilder::new()
    ///     .select(&["Id"])
    ///     .from("Contact")
    ///     .where_eq("LastName", "Smith")
    ///     .build();
    /// assert_eq!(query, "SELECT Id FROM Contact WHERE LastName = 'Smith'");
    /// ```
    pub fn try_where_eq(self, field: &str, value: &str) -> crate::error::Result<Self> {
        self.try_add_condition(field, "=", value)
    }

    /// Adds a WHERE condition for equality (e.g., `Field = 'Value'`).
    ///
    /// # Panics
    ///
    /// Panics if the field name is invalid.
    #[must_use]
    pub fn where_eq(self, field: &str, value: &str) -> Self {
        self.try_where_eq(field, value).unwrap_or_panic("where_eq")
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
    /// use force::api::SoqlQueryBuilder;
    /// let query = SoqlQueryBuilder::new()
    ///     .select(&["Id"])
    ///     .from("Contact")
    ///     .where_ne("LastName", "Smith")
    ///     .build();
    /// assert_eq!(query, "SELECT Id FROM Contact WHERE LastName != 'Smith'");
    /// ```
    pub fn try_where_ne(self, field: &str, value: &str) -> crate::error::Result<Self> {
        self.try_add_condition(field, "!=", value)
    }

    /// Adds a WHERE condition for NOT equality (e.g., `Field != 'Value'`).
    ///
    /// # Panics
    ///
    /// Panics if the field name is invalid.
    #[must_use]
    pub fn where_ne(self, field: &str, value: &str) -> Self {
        self.try_where_ne(field, value).unwrap_or_panic("where_ne")
    }

    /// Adds a simple WHERE condition (helper).
    fn try_add_condition(
        mut self,
        field: &str,
        op: &str,
        value: &str,
    ) -> crate::error::Result<Self> {
        use std::fmt::Write;

        validate_field_name(field).map_err(|e| ForceError::InvalidInput(e.to_string()))?;
        // Optimization: Use escape_soql_cow to avoid allocation if escape not needed
        let escaped_value = escape_soql_cow(value);

        // ⚡ Bolt: Avoid intermediate `format!` allocation by writing directly to a pre-allocated buffer.
        let capacity = field.len() + op.len() + escaped_value.len() + 4; // 2 spaces + 2 quotes
        let mut buffer = String::with_capacity(capacity);
        write!(buffer, "{} {} '{}'", field, op, escaped_value)
            .unwrap_or_else(|_| unreachable!("writing to String is infallible"));

        self.where_clauses.push(buffer);
        Ok(self)
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
    /// use force::api::SoqlQueryBuilder;
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
    pub fn try_where_in(
        mut self,
        field: &str,
        values: &[impl AsRef<str>],
    ) -> crate::error::Result<Self> {
        use std::fmt::Write;

        validate_field_name(field).map_err(|e| ForceError::InvalidInput(e.to_string()))?;
        if values.is_empty() {
            // ⚡ Bolt: Avoid intermediate `format!` allocation.
            let mut buffer = String::with_capacity(field.len() + 7);
            buffer.push_str(field);
            buffer.push_str(" IN ()");
            self.where_clauses.push(buffer);
            return Ok(self);
        }

        // Base capacity for "FIELD IN ()" + estimated 10 chars per value + quotes/commas
        let capacity = field.len() + 6 + (values.len() * 14);
        let mut buffer = String::with_capacity(capacity);

        write!(buffer, "{} IN (", field)
            .unwrap_or_else(|_| unreachable!("writing to String is infallible"));

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
        Ok(self)
    }

    /// Adds a WHERE condition for IN clause (e.g., `Field IN ('Val1', 'Val2')`).
    ///
    /// # Panics
    ///
    /// Panics if the field name is invalid.
    #[must_use]
    pub fn where_in(self, field: &str, values: &[impl AsRef<str>]) -> Self {
        self.try_where_in(field, values).unwrap_or_panic("where_in")
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
    /// use force::api::SoqlQueryBuilder;
    /// let query = SoqlQueryBuilder::new()
    ///     .select(&["Id"])
    ///     .from("Account")
    ///     .where_like("Name", "Acme%")
    ///     .build();
    /// assert_eq!(query, "SELECT Id FROM Account WHERE Name LIKE 'Acme%'");
    /// ```
    pub fn try_where_like(self, field: &str, value: &str) -> crate::error::Result<Self> {
        self.try_add_condition(field, "LIKE", value)
    }

    /// Adds a WHERE condition for LIKE clause (e.g., `Field LIKE 'Val%'`).
    ///
    /// # Panics
    ///
    /// Panics if the field name is invalid.
    #[must_use]
    pub fn where_like(self, field: &str, value: &str) -> Self {
        self.try_where_like(field, value)
            .unwrap_or_panic("where_like")
    }

    /// Sets the LIMIT clause.
    ///
    /// # Examples
    ///
    /// ```
    /// use force::api::SoqlQueryBuilder;
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
    /// use force::api::SoqlQueryBuilder;
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
    /// use force::api::SoqlQueryBuilder;
    /// let query = SoqlQueryBuilder::new()
    ///     .select(&["Id"])
    ///     .from("Account")
    ///     .order_by("Name")
    ///     .build();
    /// assert_eq!(query, "SELECT Id FROM Account ORDER BY Name");
    /// ```
    pub fn try_order_by(mut self, field: &str) -> crate::error::Result<Self> {
        validate_field_name(field).map_err(|e| ForceError::InvalidInput(e.to_string()))?;
        self.order_by = Some(field.to_string());
        Ok(self)
    }

    /// Sets the ORDER BY clause.
    ///
    /// # Panics
    ///
    /// Panics if the field name is invalid.
    #[must_use]
    pub fn order_by(self, field: &str) -> Self {
        self.try_order_by(field).unwrap_or_panic("order_by")
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
    /// use force::api::SoqlQueryBuilder;
    /// let query = SoqlQueryBuilder::new()
    ///     .select(&["Id"])
    ///     .from("Account")
    ///     .order_by_desc("CreatedDate")
    ///     .build();
    /// assert_eq!(query, "SELECT Id FROM Account ORDER BY CreatedDate DESC");
    /// ```
    pub fn try_order_by_desc(mut self, field: &str) -> crate::error::Result<Self> {
        validate_field_name(field).map_err(|e| ForceError::InvalidInput(e.to_string()))?;
        // ⚡ Bolt: Avoid intermediate `format!` allocation
        let mut order = String::with_capacity(field.len() + 5);
        order.push_str(field);
        order.push_str(" DESC");
        self.order_by = Some(order);
        Ok(self)
    }

    /// Sets the ORDER BY clause with direction (DESC).
    ///
    /// # Panics
    ///
    /// Panics if the field name is invalid.
    #[must_use]
    pub fn order_by_desc(self, field: &str) -> Self {
        self.try_order_by_desc(field)
            .unwrap_or_panic("order_by_desc")
    }

    /// Validates that the builder has all necessary components to build a query.
    ///
    /// # Errors
    ///
    /// Returns an error if no fields are selected or no SObject is specified.
    pub fn validate(&self) -> crate::error::Result<()> {
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
    pub fn try_build(self) -> crate::error::Result<String> {
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

    /// Builds the final SOQL query string (panicking version).
    ///
    /// # Panics
    ///
    /// Panics if no fields are selected or no SObject is specified.
    #[must_use]
    pub fn build(self) -> String {
        self.try_build().unwrap_or_panic("build")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::must::Must;

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
    #[should_panic(
        expected = "Invalid input in where_eq: invalid input: invalid input: Field name contains invalid character ';': Invalid;Field"
    )]
    fn test_where_eq_panics_on_invalid_field() {
        let _ = SoqlQueryBuilder::new().where_eq("Invalid;Field", "Value");
    }

    #[test]
    #[should_panic(
        expected = "Invalid input in where_ne: invalid input: invalid input: Field name contains invalid character ';': Invalid;Field"
    )]
    fn test_where_ne_panics_on_invalid_field() {
        let _ = SoqlQueryBuilder::new().where_ne("Invalid;Field", "Value");
    }

    #[test]
    #[should_panic(
        expected = "Invalid input in where_in: invalid input: invalid input: Field name contains invalid character ';': Invalid;Field"
    )]
    fn test_where_in_panics_on_invalid_field() {
        let _ = SoqlQueryBuilder::new().where_in("Invalid;Field", &["Value"]);
    }

    #[test]
    #[should_panic(
        expected = "Invalid input in where_like: invalid input: invalid input: Field name contains invalid character ';': Invalid;Field"
    )]
    fn test_where_like_panics_on_invalid_field() {
        let _ = SoqlQueryBuilder::new().where_like("Invalid;Field", "Value");
    }

    #[test]
    #[should_panic(
        expected = "Invalid input in order_by: invalid input: invalid input: Field name contains invalid character ';': Invalid;Field"
    )]
    fn test_order_by_panics_on_invalid_field() {
        let _ = SoqlQueryBuilder::new().order_by("Invalid;Field");
    }

    #[test]
    #[should_panic(
        expected = "Invalid input in order_by_desc: invalid input: invalid input: Field name contains invalid character ';': Invalid;Field"
    )]
    fn test_order_by_desc_panics_on_invalid_field() {
        let _ = SoqlQueryBuilder::new().order_by_desc("Invalid;Field");
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
        let result = builder.clone().try_select(&["Valid", "Invalid;DROP"]);

        if let Err(ForceError::InvalidInput(msg)) = result {
            assert!(msg.contains("invalid character"));
        } else {
            panic!("Expected ForceError::InvalidInput");
        }

        // Invalid SObject name
        let result = builder.try_from("Invalid SObject");

        if let Err(ForceError::InvalidInput(msg)) = result {
            assert!(msg.contains("invalid characters"));
        } else {
            panic!("Expected ForceError::InvalidInput");
        }
    }

    // Test unwrap_or_panic logic by calling `build` on invalid states directly.
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
    #[should_panic(
        expected = "Invalid input in from: invalid input: SObject name contains invalid characters: Invalid Object"
    )]
    fn test_from_panics_on_invalid_sobject() {
        let _ = SoqlQueryBuilder::new().from("Invalid Object");
    }

    #[test]
    #[should_panic(
        expected = "Invalid input in select: invalid input: Field name contains invalid character ';': Invalid;DROP"
    )]
    fn test_select_panics_on_invalid_field() {
        let _ = SoqlQueryBuilder::new().select(&["Valid", "Invalid;DROP"]);
    }

    #[test]
    #[should_panic(expected = "Invalid input in test_context: invalid input: test error")]
    fn test_unwrap_or_panic_helper() {
        let result: crate::error::Result<()> =
            Err(ForceError::InvalidInput("test error".to_string()));
        result.unwrap_or_panic("test_context");
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

    #[test]
    fn test_soql_from_describe() {
        use crate::test_utils::must::Must;
        use crate::types::describe::SObjectDescribe;

        // Simplify to avoid json! macro recursion limit on big objects.
        let describe_json_str = r#"{
            "name": "Account",
            "label": "Account",
            "custom": false,
            "queryable": true,
            "activateable": false, "createable": true, "customSetting": false, "deletable": true,
            "deprecatedAndHidden": false, "feedEnabled": true, "hasSubtypes": false,
            "isSubtype": false, "keyPrefix": "001", "labelPlural": "Accounts", "layoutable": true,
            "urls": {}, "childRelationships": [], "recordTypeInfos": [],
            "mergeable": true, "mruEnabled": true, "replicateable": true, "retrieveable": true,
            "searchable": true, "triggerable": true, "undeletable": true, "updateable": true,
            "fields": [
                {
                    "name": "Id",
                    "type": "id",
                    "label": "Account ID",
                    "referenceTo": [],
                    "aggregatable": true, "autoNumber": false, "byteLength": 18, "calculated": false,
                    "cascadeDelete": false, "caseSensitive": false, "createable": false, "custom": false,
                    "defaultedOnCreate": true, "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 0, "precision": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": true, "length": 18, "nameField": false, "namePointing": false, "nillable": false,
                    "permissionable": false, "polymorphicForeignKey": false, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "searchPrefilterable": false, "sortable": true,
                    "soapType": "tns:ID", "unique": false, "writeRequiresMasterRead": false, "updateable": false
                },
                {
                    "name": "Name",
                    "type": "string",
                    "label": "Account Name",
                    "referenceTo": [],
                    "aggregatable": true, "autoNumber": false, "byteLength": 255, "calculated": false,
                    "cascadeDelete": false, "caseSensitive": false, "createable": true, "custom": false,
                    "defaultedOnCreate": false, "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 0, "precision": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": false, "length": 255, "nameField": true, "namePointing": false, "nillable": false,
                    "permissionable": false, "polymorphicForeignKey": false, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "searchPrefilterable": false, "sortable": true,
                    "soapType": "xsd:string", "unique": false, "writeRequiresMasterRead": false, "updateable": true
                },
                {
                    "name": "HiddenField__c",
                    "type": "string",
                    "label": "Hidden",
                    "referenceTo": [],
                    "aggregatable": false, "autoNumber": false, "byteLength": 255, "calculated": false,
                    "cascadeDelete": false, "caseSensitive": false, "createable": false, "custom": true,
                    "defaultedOnCreate": false, "dependentPicklist": false, "deprecatedAndHidden": true,
                    "digits": 0, "precision": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": false, "groupable": false, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": false, "length": 255, "nameField": false, "namePointing": false, "nillable": true,
                    "permissionable": false, "polymorphicForeignKey": false, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "searchPrefilterable": false, "sortable": false,
                    "soapType": "xsd:string", "unique": false, "writeRequiresMasterRead": false, "updateable": false
                }
            ]
        }"#;

        let describe: SObjectDescribe = serde_json::from_str(describe_json_str).must();

        let builder = SoqlQueryBuilder::from_describe(&describe);

        // Assert sobject is correct
        assert_eq!(builder.sobject, Some("Account".to_string()));

        // Assert fields: Id and Name are included, HiddenField__c is not.
        assert_eq!(builder.fields, vec!["Id".to_string(), "Name".to_string()]);
    }
    #[test]
    #[should_panic(
        expected = "Invalid input in where_eq: invalid input: invalid input: Field name contains invalid character ';': Invalid;Field"
    )]
    fn test_where_eq_panics_on_invalid_field_semicolon() {
        let _ = SoqlQueryBuilder::new().where_eq("Invalid;Field", "Value");
    }
}
