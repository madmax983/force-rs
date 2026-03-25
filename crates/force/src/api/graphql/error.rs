//! GraphQL-specific error types.

use super::types::GraphqlError;
use std::fmt;

/// Wrapper for one or more GraphQL errors returned by the Salesforce GraphQL API.
///
/// This type is used as the payload for [`ForceError::GraphQL`](crate::error::ForceError::GraphQL)
/// when a GraphQL response contains errors without data.
#[derive(Debug, Clone)]
pub struct GraphqlErrorResponse(pub Vec<GraphqlError>);

impl fmt::Display for GraphqlErrorResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // ⚡ Bolt: Write error messages directly to the formatter to avoid allocating
        // an intermediate Vec and a concatenated String via `.join()`.
        for (i, err) in self.0.iter().enumerate() {
            if i > 0 {
                write!(f, "; ")?;
            }
            write!(f, "{}", err.message)?;
        }
        Ok(())
    }
}

impl std::error::Error for GraphqlErrorResponse {}

impl GraphqlErrorResponse {
    /// Returns the individual GraphQL errors.
    #[must_use]
    pub fn errors(&self) -> &[GraphqlError] {
        &self.0
    }

    /// Returns the number of errors.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns `true` if there are no errors.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_error(msg: &str) -> GraphqlError {
        GraphqlError {
            message: msg.to_string(),
            locations: vec![],
            path: vec![],
            extensions: HashMap::new(),
        }
    }

    #[test]
    fn test_display_single_error() {
        let resp = GraphqlErrorResponse(vec![make_error("Field not found")]);
        assert_eq!(resp.to_string(), "Field not found");
    }

    #[test]
    fn test_display_multiple_errors() {
        let resp = GraphqlErrorResponse(vec![
            make_error("Field not found"),
            make_error("Insufficient access"),
        ]);
        assert_eq!(resp.to_string(), "Field not found; Insufficient access");
    }

    #[test]
    fn test_display_empty_errors() {
        let resp = GraphqlErrorResponse(vec![]);
        assert_eq!(resp.to_string(), "");
        assert!(resp.is_empty());
    }

    #[test]
    fn test_error_trait_is_implemented() {
        fn assert_error<T: std::error::Error>() {}
        assert_error::<GraphqlErrorResponse>();
    }

    #[test]
    fn test_errors_accessor() {
        let resp = GraphqlErrorResponse(vec![make_error("oops")]);
        assert_eq!(resp.errors().len(), 1);
        assert_eq!(resp.len(), 1);
        assert!(!resp.is_empty());
    }

    #[test]
    fn test_conversion_to_force_error() {
        let resp = GraphqlErrorResponse(vec![make_error("bad query")]);
        let force_err: crate::error::ForceError = resp.into();
        assert!(
            force_err.to_string().contains("bad query"),
            "ForceError should contain the GraphQL error message"
        );
    }
}
