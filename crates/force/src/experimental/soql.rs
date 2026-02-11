//! SOQL Query Builder.
//!
//! A type-safe builder for constructing SOQL queries.

/// A builder for SOQL queries.
///
/// # Examples
///
/// ```rust
/// use force::experimental::soql::SoqlQuery;
///
/// let query = SoqlQuery::new("Account")
///     .select(&["Id", "Name"])
///     .where_eq("Type", "Customer")
///     .limit(10)
///     .build();
///
/// assert_eq!(query, "SELECT Id, Name FROM Account WHERE Type = 'Customer' LIMIT 10");
/// ```
#[derive(Debug, Clone, Default)]
pub struct SoqlQuery {
    object: String,
    fields: Vec<String>,
    conditions: Vec<String>,
    limit: Option<u32>,
    offset: Option<u32>,
    order_by: Option<String>,
}

impl SoqlQuery {
    /// Creates a new SOQL query builder for the specified object.
    #[must_use]
    pub fn new(object: &str) -> Self {
        Self {
            object: object.to_string(),
            fields: Vec::new(),
            conditions: Vec::new(),
            limit: None,
            offset: None,
            order_by: None,
        }
    }

    /// Adds fields to the SELECT clause.
    #[must_use]
    pub fn select(mut self, fields: &[&str]) -> Self {
        for field in fields {
            self.fields.push(field.to_string());
        }
        self
    }

    /// Adds a WHERE clause condition for equality.
    ///
    /// Note: This does simplistic string escaping. For complex queries, be careful.
    #[must_use]
    pub fn where_eq(mut self, field: &str, value: &str) -> Self {
        // Basic escaping of single quotes
        let escaped_value = value.replace('\'', "\\'");
        self.conditions
            .push(format!("{} = '{}'", field, escaped_value));
        self
    }

    /// Adds a raw WHERE clause condition.
    #[must_use]
    pub fn where_raw(mut self, condition: &str) -> Self {
        self.conditions.push(condition.to_string());
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
    pub fn order_by(mut self, field: &str) -> Self {
        self.order_by = Some(field.to_string());
        self
    }

    /// Builds the SOQL query string.
    #[must_use]
    pub fn build(self) -> String {
        let fields = if self.fields.is_empty() {
            "Id".to_string() // Default to Id if no fields specified
        } else {
            self.fields.join(", ")
        };

        let mut query = format!("SELECT {} FROM {}", fields, self.object);

        if !self.conditions.is_empty() {
            query.push_str(" WHERE ");
            query.push_str(&self.conditions.join(" AND "));
        }

        if let Some(order) = self.order_by {
            query.push_str(&format!(" ORDER BY {}", order));
        }

        if let Some(limit) = self.limit {
            query.push_str(&format!(" LIMIT {}", limit));
        }

        if let Some(offset) = self.offset {
            query.push_str(&format!(" OFFSET {}", offset));
        }

        query
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_query() {
        let query = SoqlQuery::new("Account").select(&["Id", "Name"]).build();
        assert_eq!(query, "SELECT Id, Name FROM Account");
    }

    #[test]
    fn test_where_clause() {
        let query = SoqlQuery::new("Contact")
            .select(&["FirstName", "LastName"])
            .where_eq("Email", "test@example.com")
            .build();
        assert_eq!(
            query,
            "SELECT FirstName, LastName FROM Contact WHERE Email = 'test@example.com'"
        );
    }

    #[test]
    fn test_complex_query() {
        let query = SoqlQuery::new("Opportunity")
            .select(&["Name", "Amount"])
            .where_eq("StageName", "Closed Won")
            .limit(5)
            .offset(10)
            .order_by("Amount DESC")
            .build();

        assert_eq!(
            query,
            "SELECT Name, Amount FROM Opportunity WHERE StageName = 'Closed Won' ORDER BY Amount DESC LIMIT 5 OFFSET 10"
        );
    }

    #[test]
    fn test_escaping() {
        let query = SoqlQuery::new("Account")
            .select(&["Id"])
            .where_eq("Name", "O'Reilly")
            .build();
        assert_eq!(query, "SELECT Id FROM Account WHERE Name = 'O\\'Reilly'");
    }
}
