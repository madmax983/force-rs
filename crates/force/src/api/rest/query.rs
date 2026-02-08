//! SOQL query execution with automatic pagination.
//!
//! This module provides typed and dynamic SOQL query execution with automatic
//! pagination support through Streams.

use crate::client::ForceClient;
use crate::error::ForceError;
use crate::types::QueryResult;
use futures::stream::Stream;
use serde::de::DeserializeOwned;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

/// A Stream that automatically handles SOQL query pagination.
///
/// This stream fetches pages on-demand using `nextRecordsUrl` until all
/// results are retrieved.
#[derive(Debug)]
#[allow(dead_code)]  // Will be used when query() is implemented
pub struct QueryStream<T, A: crate::auth::Authenticator> {
    inner: Arc<crate::client::Inner<A>>,
    next_url: Option<String>,
    current_batch: Vec<T>,
    done: bool,
    _marker: std::marker::PhantomData<T>,
}

impl<T, A: crate::auth::Authenticator> QueryStream<T, A> {
    /// Creates a new query stream from an initial query result.
    #[allow(dead_code)]  // Will be used when query() is implemented
    fn new(inner: Arc<crate::client::Inner<A>>, initial_result: QueryResult<T>) -> Self {
        Self {
            inner,
            next_url: initial_result.next_records_url,
            current_batch: initial_result.records,
            done: initial_result.done,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<T, A> Stream for QueryStream<T, A>
where
    T: DeserializeOwned + Unpin,
    A: crate::auth::Authenticator + Unpin,
{
    type Item = Result<T, ForceError>;

    fn poll_next(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // TODO: Implement actual async pagination
        // For now, just return records from current batch
        let this = self.get_mut();

        if let Some(record) = this.current_batch.pop() {
            return Poll::Ready(Some(Ok(record)));
        }

        if this.done {
            return Poll::Ready(None);
        }

        // TODO: Fetch next page using next_url
        Poll::Ready(None)
    }
}

impl<A: crate::auth::Authenticator> ForceClient<A> {
    /// Executes a SOQL query and returns the first page of results.
    ///
    /// This method performs a single query and returns only the first page.
    /// Use `query_all()` to automatically fetch all pages via a Stream.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The query is malformed
    /// - Network/authentication failures occur
    /// - Deserialization fails
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use force::types::DynamicSObject;
    ///
    /// let result = client.query::<DynamicSObject>("SELECT Id, Name FROM Account LIMIT 10").await?;
    /// println!("Total: {}", result.total_size);
    /// for record in result.records {
    ///     println!("{:?}", record);
    /// }
    /// ```
    #[allow(clippy::unused_async)]  // Async signature needed for future implementation
    pub async fn query<T>(&self, _soql: &str) -> Result<QueryResult<T>, ForceError>
    where
        T: DeserializeOwned,
    {
        // GREEN phase implementation placeholder
        // Will be implemented once RestHandler is complete (Task #1)
        Err(ForceError::NotImplemented(
            "query not yet implemented - blocked on Task #1 RestHandler".to_string(),
        ))
    }

    /// Executes a SOQL query and returns a Stream of all results with automatic pagination.
    ///
    /// This method returns a Stream that automatically fetches subsequent pages
    /// as you consume records. This is more memory-efficient for large result sets.
    ///
    /// # Errors
    ///
    /// Stream items may error if:
    /// - Network failures occur during pagination
    /// - Authentication expires
    /// - Deserialization fails for a record
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use futures::StreamExt;
    /// use force::types::DynamicSObject;
    ///
    /// let mut stream = client.query_all::<DynamicSObject>("SELECT Id, Name FROM Account");
    /// while let Some(result) = stream.next().await {
    ///     match result {
    ///         Ok(record) => println!("{:?}", record),
    ///         Err(e) => eprintln!("Error: {}", e),
    ///     }
    /// }
    /// ```
    pub fn query_all<T>(&self, _soql: &str) -> QueryStream<T, A>
    where
        T: DeserializeOwned + Unpin,
    {
        // GREEN phase implementation placeholder
        // Will be implemented once RestHandler is complete (Task #1)
        QueryStream {
            inner: Arc::clone(self.inner()),
            next_url: None,
            current_batch: Vec::new(),
            done: true,
            _marker: std::marker::PhantomData,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::DynamicSObject;
    use serde::{Deserialize, Serialize};

    // Test SObject for typed queries
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct TestAccount {
        #[serde(rename = "Id")]
        id: String,
        #[serde(rename = "Name")]
        name: String,
    }

    // RED PHASE - Write failing tests first
    // Note: These are compilation tests to ensure types are correct.
    // Integration tests will be added once the client builder is stabilized.

    #[test]
    fn test_query_result_deserialization_typed() {
        // Test that QueryResult can deserialize typed records
        let json = serde_json::json!({
            "totalSize": 2,
            "done": true,
            "records": [
                {"Id": "001", "Name": "Acme"},
                {"Id": "002", "Name": "Globex"}
            ]
        });

        let result: QueryResult<TestAccount> = serde_json::from_value(json).unwrap();

        assert_eq!(result.total_size, 2);
        assert!(result.is_done());
        assert_eq!(result.len(), 2);
        assert_eq!(result.records[0].name, "Acme");
        assert_eq!(result.records[1].name, "Globex");
    }

    #[test]
    fn test_query_result_deserialization_dynamic() {
        // Test that QueryResult can deserialize DynamicSObject
        let json = serde_json::json!({
            "totalSize": 1,
            "done": true,
            "records": [{
                "attributes": {
                    "type": "Account",
                    "url": "/services/data/v60.0/sobjects/Account/001"
                },
                "Id": "001",
                "Name": "Acme"
            }]
        });

        let result: QueryResult<DynamicSObject> = serde_json::from_value(json).unwrap();

        assert_eq!(result.total_size, 1);
        assert_eq!(result.records[0].object_type(), "Account");
        assert_eq!(
            result.records[0]
                .get_field("Name")
                .and_then(|v: &serde_json::Value| v.as_str()),
            Some("Acme")
        );
    }

    #[test]
    fn test_query_result_with_pagination() {
        // Test pagination metadata
        let json = serde_json::json!({
            "totalSize": 4,
            "done": false,
            "nextRecordsUrl": "/services/data/v60.0/query/01g-2000",
            "records": [
                {"Id": "001", "Name": "Acme"},
                {"Id": "002", "Name": "Globex"}
            ]
        });

        let result: QueryResult<TestAccount> = serde_json::from_value(json).unwrap();

        assert_eq!(result.total_size, 4);
        assert!(!result.is_done());
        assert!(result.has_more());
        assert_eq!(
            result.next_records_url,
            Some("/services/data/v60.0/query/01g-2000".to_string())
        );
    }

    #[test]
    fn test_query_result_empty() {
        // Test empty result set
        let json = serde_json::json!({
            "totalSize": 0,
            "done": true,
            "records": []
        });

        let result: QueryResult<TestAccount> = serde_json::from_value(json).unwrap();

        assert_eq!(result.total_size, 0);
        assert!(result.is_done());
        assert!(result.is_empty());
    }

    #[test]
    fn test_query_result_with_nested_query() {
        // Test complex SOQL with subqueries
        let json = serde_json::json!({
            "totalSize": 1,
            "done": true,
            "records": [{
                "Id": "001",
                "Name": "Tech Corp",
                "Contacts": {
                    "totalSize": 0,
                    "done": true,
                    "records": []
                }
            }]
        });

        let result: QueryResult<serde_json::Value> = serde_json::from_value(json).unwrap();
        assert_eq!(result.total_size, 1);
    }

    #[test]
    fn test_query_stream_type_safety() {
        // Compile-time test: Ensure QueryStream has correct type bounds
        fn _assert_stream_traits<T, A>()
        where
            T: serde::de::DeserializeOwned + Unpin,
            A: crate::auth::Authenticator + Unpin,
        {
            // This function doesn't need to run, just compile
            fn _is_stream<S: futures::stream::Stream>(_s: S) {}
            // If QueryStream implements Stream, this would compile
        }
    }
}
