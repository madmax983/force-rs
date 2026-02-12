//! Fluent SOQL Query Builder.
//!
//! This module provides a type-safe builder for constructing SOQL queries.
//!
//! # Examples
//!
//! ```
//! use force::experimental::soql::SoqlQuery;
//!
//! let query = SoqlQuery::new()
//!     .select(&["Id", "Name", "BillingCity"])
//!     .from("Account")
//!     .where_eq("Type", "Customer - Direct")
//!     .limit(10)
//!     .build()
//!     .unwrap();
//!
//! assert_eq!(
//!     query,
//!     "SELECT Id, Name, BillingCity FROM Account WHERE Type = 'Customer - Direct' LIMIT 10"
//! );
//! ```

/// Error type for SOQL query building.
#[derive(Debug, thiserror::Error)]
#[error("SOQL error: {0}")]
pub struct SoqlError(String);

/// A builder for SOQL queries.
#[derive(Debug, Clone, Default)]
pub struct SoqlQuery {
    fields: Vec<String>,
    object: Option<String>,
    conditions: Vec<String>,
    limit: Option<u32>,
    offset: Option<u32>,
    order_by: Option<String>,
}

impl SoqlQuery {
    /// Creates a new empty SOQL query builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the fields to select.
    #[must_use]
    pub fn select<I, S>(mut self, fields: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: ToString,
    {
        self.fields = fields.into_iter().map(|s| s.to_string()).collect();
        self
    }

    /// Sets the object to select from.
    #[must_use]
    pub fn from(mut self, object: &str) -> Self {
        self.object = Some(object.to_string());
        self
    }

    /// Adds a WHERE condition.
    /// Multiple calls will be joined with AND.
    #[must_use]
    pub fn where_condition(mut self, condition: &str) -> Self {
        self.conditions.push(condition.to_string());
        self
    }

    /// Adds a WHERE equality condition with value escaping.
    #[must_use]
    pub fn where_eq(mut self, field: &str, value: &str) -> Self {
        let escaped = value.replace('\\', "\\\\").replace('\'', "\\'");
        self.conditions.push(format!("{} = '{}'", field, escaped));
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
    #[must_use]
    pub fn order_by(mut self, order_by: &str) -> Self {
        self.order_by = Some(order_by.to_string());
        self
    }

    /// Builds the SOQL query string.
    ///
    /// # Errors
    ///
    /// Returns an error if the object (FROM clause) is not set.
    pub fn build(self) -> Result<String, SoqlError> {
        let object = self
            .object
            .ok_or_else(|| SoqlError("SOQL query must have a FROM clause".to_string()))?;

        let fields = if self.fields.is_empty() {
            "Id".to_string()
        } else {
            self.fields.join(", ")
        };

        let mut query = format!("SELECT {} FROM {}", fields, object);

        if !self.conditions.is_empty() {
            query.push_str(" WHERE ");
            query.push_str(&self.conditions.join(" AND "));
        }

        if let Some(order_by) = self.order_by {
            query.push_str(&format!(" ORDER BY {}", order_by));
        }

        if let Some(limit) = self.limit {
            query.push_str(&format!(" LIMIT {}", limit));
        }

        if let Some(offset) = self.offset {
            query.push_str(&format!(" OFFSET {}", offset));
        }

        Ok(query)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_query() {
        let query = SoqlQuery::new()
            .select(&["Id", "Name"])
            .from("Account")
            .build()
            .unwrap();

        assert_eq!(query, "SELECT Id, Name FROM Account");
    }

    #[test]
    fn test_escaping_backslash() {
        let query = SoqlQuery::new()
            .select(&["Id"])
            .from("Document")
            .where_eq("Path", "C:\\Windows\\System32")
            .build()
            .unwrap();

        assert_eq!(
            query,
            "SELECT Id FROM Document WHERE Path = 'C:\\\\Windows\\\\System32'"
        );
    }

    #[test]
    fn test_where_clause() {
        let query = SoqlQuery::new()
            .select(&["Id"])
            .from("Contact")
            .where_eq("LastName", "Doe")
            .build()
            .unwrap();

        assert_eq!(query, "SELECT Id FROM Contact WHERE LastName = 'Doe'");
    }

    #[test]
    fn test_complex_query() {
        let query = SoqlQuery::new()
            .select(&["Id", "Name"])
            .from("Opportunity")
            .where_eq("StageName", "Closed Won")
            .where_condition("Amount > 10000")
            .order_by("Amount DESC")
            .limit(5)
            .build()
            .unwrap();

        assert_eq!(
            query,
            "SELECT Id, Name FROM Opportunity WHERE StageName = 'Closed Won' AND Amount > 10000 ORDER BY Amount DESC LIMIT 5"
        );
    }

    #[test]
    fn test_escaping() {
        let query = SoqlQuery::new()
            .select(&["Id"])
            .from("Account")
            .where_eq("Name", "O'Reilly")
            .build()
            .unwrap();

        assert_eq!(query, "SELECT Id FROM Account WHERE Name = 'O\\'Reilly'");
    }

    #[test]
    fn test_missing_from() {
        let result = SoqlQuery::new().select(&["Id"]).build();
        assert!(result.is_err());
    }
}
