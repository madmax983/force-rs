//! Asynchronous stream for iterating over SOQL query results.
//!
//! This module provides the `QueryStream` struct to simplify paginated SOQL queries.
//!
//! # Examples
//!
//! ```ignore
//! use futures::StreamExt;
//!
//! let mut stream = client.rest().query_stream::<Account>("SELECT Id, Name FROM Account");
//!
//! while let Some(result) = stream.next().await {
//!     match result {
//!         Ok(record) => println!("Found account: {}", record.name),
//!         Err(e) => eprintln!("Error: {}", e),
//!     }
//! }
//! ```

use super::RestHandler;
use crate::auth::Authenticator;
use crate::error::Result;
use futures::Stream;
use serde::de::DeserializeOwned;

/// Stream for iterating over query results.
///
/// This stream lazily fetches and deserializes results from a SOQL query,
/// automatically handling pagination via `query` and `query_more`.
pub struct QueryStream<T, A: Authenticator> {
    client: RestHandler<A>,
    soql: String,
    current_page: std::vec::IntoIter<T>,
    next_url: Option<String>,
    done: bool,
    started: bool,
    exhausted: bool,
}

impl<T, A> QueryStream<T, A>
where
    T: DeserializeOwned + Unpin,
    A: Authenticator,
{
    /// Creates a new query stream.
    pub fn new(client: RestHandler<A>, soql: impl Into<String>) -> Self {
        Self {
            client,
            soql: soql.into(),
            current_page: Vec::new().into_iter(),
            next_url: None,
            done: false,
            started: false,
            exhausted: false,
        }
    }

    /// Fetches the next record from the stream.
    ///
    /// Returns `None` when all records have been consumed.
    pub async fn next(&mut self) -> Result<Option<T>> {
        loop {
            // 1. Try to yield from current page
            if let Some(record) = self.current_page.next() {
                return Ok(Some(record));
            }

            // 2. If exhausted, return None
            if self.exhausted {
                return Ok(None);
            }

            // 3. Current page is empty, fetch more
            let result = if !self.started {
                self.started = true;
                self.client.query::<T>(&self.soql).await?
            } else if !self.done {
                if let Some(ref url) = self.next_url {
                    self.client.query_more::<T>(url).await?
                } else {
                    // Done is false but no URL? Treat as done to avoid infinite loop.
                    self.exhausted = true;
                    return Ok(None);
                }
            } else {
                // Started, current page empty, done = true -> exhausted
                self.exhausted = true;
                return Ok(None);
            };

            // 4. Update state
            // Optimization: Use IntoIter to avoid moving elements into a VecDeque
            self.current_page = result.records.into_iter();
            self.next_url = result.next_records_url;
            self.done = result.done;

            // 5. If fetch returned nothing and we are done, mark exhausted.
            // If fetch returned nothing but not done (weird), loop again to fetch next page.
            // Note: self.current_page.len() requires ExactSizeIterator which IntoIter is.
            if self.current_page.len() == 0 && self.done {
                self.exhausted = true;
                return Ok(None);
            }
        }
    }

    /// Converts this query stream into a `futures::Stream`.
    pub fn into_stream(self) -> impl Stream<Item = Result<T>> {
        futures::stream::unfold(self, |mut stream| async move {
            match stream.next().await {
                Ok(Some(item)) => Some((Ok(item), stream)),
                Ok(None) => None,
                Err(e) => Some((Err(e), stream)),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{AccessToken, Authenticator, TokenResponse};
    use crate::client::builder;
    use crate::test_support::Must;
    use async_trait::async_trait;
    use futures::StreamExt;
    use serde::{Deserialize, Serialize};
    use serde_json::json;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    // Mock authenticator for testing
    #[derive(Debug, Clone)]
    struct MockAuthenticator {
        token: String,
        instance_url: String,
    }

    impl MockAuthenticator {
        fn new(token: &str, instance_url: &str) -> Self {
            Self {
                token: token.to_string(),
                instance_url: instance_url.to_string(),
            }
        }
    }

    #[async_trait]
    impl Authenticator for MockAuthenticator {
        async fn authenticate(&self) -> Result<AccessToken> {
            Ok(AccessToken::from_response(TokenResponse {
                access_token: self.token.clone(),
                instance_url: self.instance_url.clone(),
                token_type: "Bearer".to_string(),
                issued_at: "1704067200000".to_string(),
                signature: "test_sig".to_string(),
                expires_in: Some(7200),
                refresh_token: None,
            }))
        }

        async fn refresh(&self) -> Result<AccessToken> {
            self.authenticate().await
        }
    }

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct TestAccount {
        #[serde(rename = "Id")]
        id: String,
        #[serde(rename = "Name")]
        name: String,
    }

    #[tokio::test]
    async fn test_query_stream_single_page() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/query"))
            .and(query_param("q", "SELECT Id, Name FROM Account"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "totalSize": 2,
                "done": true,
                "records": [
                    {"Id": "001", "Name": "A"},
                    {"Id": "002", "Name": "B"}
                ]
            })))
            .mount(&mock_server)
            .await;

        let client = builder().authenticate(auth).build().await.must();
        // NOTE: This relies on RestHandler having query_stream method, which will be added in mod.rs
        let mut stream = client
            .rest()
            .query_stream::<TestAccount>("SELECT Id, Name FROM Account");

        let r1 = stream.next().await.must().must();
        assert_eq!(r1.name, "A");

        let r2 = stream.next().await.must().must();
        assert_eq!(r2.name, "B");

        assert!(stream.next().await.must().is_none());
    }

    #[tokio::test]
    async fn test_query_stream_multi_page() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/query"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "totalSize": 4,
                "done": false,
                "nextRecordsUrl": "/services/data/v60.0/query/next",
                "records": [
                    {"Id": "001", "Name": "A"},
                    {"Id": "002", "Name": "B"}
                ]
            })))
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/query/next"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "totalSize": 4,
                "done": true,
                "records": [
                    {"Id": "003", "Name": "C"},
                    {"Id": "004", "Name": "D"}
                ]
            })))
            .mount(&mock_server)
            .await;

        let client = builder().authenticate(auth).build().await.must();
        let stream = client
            .rest()
            .query_stream::<TestAccount>("SELECT Id, Name FROM Account");

        let results: Vec<_> = stream.into_stream().collect().await;
        assert_eq!(results.len(), 4);
        assert_eq!(results[0].as_ref().must().name, "A");
        assert_eq!(results[3].as_ref().must().name, "D");
    }

    #[tokio::test]
    async fn test_query_stream_empty() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/query"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "totalSize": 0,
                "done": true,
                "records": []
            })))
            .mount(&mock_server)
            .await;

        let client = builder().authenticate(auth).build().await.must();
        let mut stream = client
            .rest()
            .query_stream::<TestAccount>("SELECT Id, Name FROM Account");

        assert!(stream.next().await.must().is_none());
    }

    #[tokio::test]
    async fn test_query_stream_error() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/query"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock_server)
            .await;

        let client = builder().authenticate(auth).build().await.must();
        let mut stream = client
            .rest()
            .query_stream::<TestAccount>("SELECT Id, Name FROM Account");

        assert!(stream.next().await.is_err());
    }

    #[tokio::test]
    async fn test_query_stream_pagination_error() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());

        // First page succeeds
        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/query"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "totalSize": 4,
                "done": false,
                "nextRecordsUrl": "/services/data/v60.0/query/next",
                "records": [
                    {"Id": "001", "Name": "A"}
                ]
            })))
            .mount(&mock_server)
            .await;

        // Second page fails
        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/query/next"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock_server)
            .await;

        let client = builder().authenticate(auth).build().await.must();
        let mut stream = client
            .rest()
            .query_stream::<TestAccount>("SELECT Id, Name FROM Account");

        // Should get first record
        let r1 = stream.next().await.must().must();
        assert_eq!(r1.name, "A");

        // Should fail on second fetch
        assert!(stream.next().await.is_err());
    }

    #[tokio::test]
    async fn test_query_stream_empty_middle_page() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());

        // First page: returns 1 record, done=false
        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/query"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "totalSize": 3,
                "done": false,
                "nextRecordsUrl": "/services/data/v60.0/query/page2",
                "records": [
                    {"Id": "001", "Name": "A"}
                ]
            })))
            .mount(&mock_server)
            .await;

        // Second page: returns 0 records, done=false (empty page)
        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/query/page2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "totalSize": 3,
                "done": false,
                "nextRecordsUrl": "/services/data/v60.0/query/page3",
                "records": []
            })))
            .mount(&mock_server)
            .await;

        // Third page: returns 1 record, done=true
        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/query/page3"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "totalSize": 3,
                "done": true,
                "records": [
                    {"Id": "002", "Name": "B"}
                ]
            })))
            .mount(&mock_server)
            .await;

        let client = builder().authenticate(auth).build().await.must();
        let mut stream = client
            .rest()
            .query_stream::<TestAccount>("SELECT Id, Name FROM Account");

        // Should get first record
        let r1 = stream.next().await.must().must();
        assert_eq!(r1.name, "A");

        // Should automatically skip empty page and get second record
        let r2 = stream.next().await.must().must();
        assert_eq!(r2.name, "B");

        assert!(stream.next().await.must().is_none());
    }
}
