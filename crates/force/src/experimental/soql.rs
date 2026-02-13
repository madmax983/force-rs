//! Type-safe SOQL query builder.
//!
//! This module provides a builder for constructing SOQL queries safely.
//!
//! # Examples
//!
//! ```
//! use force::experimental::soql::SoqlQuery;
//!
//! let query = SoqlQuery::new()
//!     .select(&["Id", "Name"])
//!     .from("Account")
//!     .limit(5)
//!     .build()
//!     .unwrap();
//!
//! assert_eq!(query, "SELECT Id, Name FROM Account LIMIT 5");
//! ```

use thiserror::Error;

/// Error type for SOQL query construction.
#[derive(Debug, Error)]
pub enum SoqlError {
    /// No fields were selected.
    #[error("Missing SELECT fields")]
    MissingFields,
    /// No object was specified.
    #[error("Missing FROM object")]
    MissingObject,
}

/// A builder for SOQL queries.
#[derive(Debug, Default, Clone)]
pub struct SoqlQuery {
    fields: Vec<String>,
    object: Option<String>,
    where_clauses: Vec<String>,
    limit: Option<usize>,
    offset: Option<usize>,
}

impl SoqlQuery {
    /// Creates a new SOQL query builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Selects fields to retrieve.
    #[must_use]
    pub fn select(mut self, fields: &[&str]) -> Self {
        self.fields.extend(fields.iter().map(|s| s.to_string()));
        self
    }

    /// Sets the object to query from.
    #[must_use]
    pub fn from(mut self, object: &str) -> Self {
        self.object = Some(object.to_string());
        self
    }

    /// Adds a WHERE clause condition.
    /// multiple calls to `where_cond` will be joined by `AND`.
    #[must_use]
    pub fn where_cond(mut self, condition: &str) -> Self {
        self.where_clauses.push(condition.to_string());
        self
    }

    /// Sets the maximum number of records to return.
    #[must_use]
    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Sets the offset for the query.
    #[must_use]
    pub fn offset(mut self, offset: usize) -> Self {
        self.offset = Some(offset);
        self
    }

    /// Builds the SOQL query string.
    ///
    /// # Errors
    ///
    /// Returns `SoqlError` if required parts (SELECT, FROM) are missing.
    pub fn build(self) -> Result<String, SoqlError> {
        if self.fields.is_empty() {
            return Err(SoqlError::MissingFields);
        }
        let object = self.object.ok_or(SoqlError::MissingObject)?;

        let mut query = format!("SELECT {} FROM {}", self.fields.join(", "), object);

        if !self.where_clauses.is_empty() {
            query.push_str(" WHERE ");
            query.push_str(&self.where_clauses.join(" AND "));
        }

        if let Some(limit) = self.limit {
            query.push_str(" LIMIT ");
            query.push_str(&limit.to_string());
        }

        if let Some(offset) = self.offset {
            query.push_str(" OFFSET ");
            query.push_str(&offset.to_string());
        }

        Ok(query)
    }
}

/// Helper function to escape single quotes in SOQL strings.
///
/// This escapes `'` to `\'` and `\` to `\\`.
#[must_use]
pub fn escape_string(input: &str) -> String {
    input.replace('\\', "\\\\").replace('\'', "\\'")
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_query() {
        let query = SoqlQuery::new()
            .select(&["Id", "Name"])
            .from("Account")
            .build()
            .expect("Failed to build query");
        assert_eq!(query, "SELECT Id, Name FROM Account");
    }

    #[test]
    fn test_where_clause() {
        let query = SoqlQuery::new()
            .select(&["Id"])
            .from("Contact")
            .where_cond("Name = 'John'")
            .build()
            .expect("Failed to build query");
        assert_eq!(query, "SELECT Id FROM Contact WHERE Name = 'John'");
    }

    #[test]
    fn test_multiple_where_clauses() {
        let query = SoqlQuery::new()
            .select(&["Id"])
            .from("Contact")
            .where_cond("Name = 'John'")
            .where_cond("Age > 25")
            .build()
            .expect("Failed to build query");
        assert_eq!(
            query,
            "SELECT Id FROM Contact WHERE Name = 'John' AND Age > 25"
        );
    }

    #[test]
    fn test_limit_and_offset() {
        let query = SoqlQuery::new()
            .select(&["Id"])
            .from("Opportunity")
            .limit(10)
            .offset(5)
            .build()
            .expect("Failed to build query");
        assert_eq!(query, "SELECT Id FROM Opportunity LIMIT 10 OFFSET 5");
    }

    #[test]
    fn test_missing_fields() {
        let err = SoqlQuery::new()
            .from("Account")
            .build()
            .expect_err("Expected error");
        assert!(matches!(err, SoqlError::MissingFields));
    }

    #[test]
    fn test_missing_object() {
        let err = SoqlQuery::new()
            .select(&["Id"])
            .build()
            .expect_err("Expected error");
        assert!(matches!(err, SoqlError::MissingObject));
    }

    #[test]
    fn test_escape_string() {
        assert_eq!(escape_string("O'Neil"), "O\\'Neil");
        assert_eq!(escape_string("Back\\Slash"), "Back\\\\Slash");
        assert_eq!(escape_string("Mixed ' and \\"), "Mixed \\' and \\\\");
    }
}
