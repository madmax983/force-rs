//! REST API handler for Salesforce.
//!
//! This module provides the `RestHandler` which serves as the foundation for all
//! REST API operations including CRUD, queries, and metadata operations.

pub mod crud;
pub mod describe;
pub mod limits;
pub mod query;
pub mod search;

use crate::error::Result;
use serde::de::DeserializeOwned;
use std::sync::Arc;

/// REST API handler for performing Salesforce REST operations.
///
/// The handler provides access to all REST API functionality including:
/// - CRUD operations (Create, Read, Update, Delete, Upsert)
/// - SOQL queries with pagination
/// - SOSL searches
/// - Metadata operations (Describe API)
/// - Org limits
///
/// The handler is obtained from a `ForceClient` and shares its authentication
/// and configuration.
#[derive(Debug, Clone)]
pub struct RestHandler<A: crate::auth::Authenticator> {
    /// Reference to the client's inner state.
    inner: Arc<crate::client::Inner<A>>,
}

impl<A: crate::auth::Authenticator> RestHandler<A> {
    /// Creates a new REST handler for the given client inner state.
    ///
    /// # Arguments
    ///
    /// * `inner` - The client's inner state
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let handler = RestHandler::new(inner);
    /// ```
    #[must_use]
    pub(crate) fn new(inner: Arc<crate::client::Inner<A>>) -> Self {
        Self { inner }
    }

    /// Constructs the base URL for REST API operations.
    ///
    /// The base URL is constructed as: `{instance_url}/services/data/{api_version}`
    ///
    /// This method requires token access to get the instance URL from authentication.
    ///
    /// # Errors
    ///
    /// Returns an error if token retrieval fails.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let base = handler.base_url().await?;
    /// // Returns: "https://na1.salesforce.com/services/data/v60.0"
    /// ```
    pub async fn base_url(&self) -> Result<String> {
        let token = self.inner.token_manager.token().await?;
        Ok(format!(
            "{}/services/data/{}",
            token.instance_url(),
            self.inner.config.api_version
        ))
    }

    /// Helper method to execute a GET request and deserialize the response.
    pub(crate) async fn execute_get<T: DeserializeOwned>(
        &self,
        path: &str,
        query: Option<&[(&str, &str)]>,
        error_msg: &str,
    ) -> Result<T> {
        let url = format!("{}{}", self.base_url().await?, path);
        let mut request = self.inner.http_client.get(&url);

        if let Some(params) = query {
            request = request.query(params);
        }

        let request = request.build().map_err(crate::error::HttpError::from)?;
        self.inner.send_request_and_decode(request, error_msg).await
    }

    /// Helper method to execute a POST request and deserialize the response.
    pub(crate) async fn execute_post<T: DeserializeOwned>(
        &self,
        path: &str,
        body: &serde_json::Value,
        error_msg: &str,
    ) -> Result<T> {
        let url = format!("{}{}", self.base_url().await?, path);
        let request = self
            .inner
            .http_client
            .post(&url)
            .json(body)
            .build()
            .map_err(crate::error::HttpError::from)?;
        self.inner.send_request_and_decode(request, error_msg).await
    }

    /// Helper method to execute a PATCH request and expect an empty success response.
    pub(crate) async fn execute_patch_empty(
        &self,
        path: &str,
        body: &serde_json::Value,
        error_msg: &str,
    ) -> Result<()> {
        let url = format!("{}{}", self.base_url().await?, path);
        let request = self
            .inner
            .http_client
            .patch(&url)
            .json(body)
            .build()
            .map_err(crate::error::HttpError::from)?;
        let response = self.inner.execute_request(request).await?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(crate::http::response_to_force_error(response, error_msg).await)
        }
    }

    /// Helper method to execute a DELETE request and expect an empty success response.
    pub(crate) async fn execute_delete_empty(&self, path: &str, error_msg: &str) -> Result<()> {
        let url = format!("{}{}", self.base_url().await?, path);
        let request = self
            .inner
            .http_client
            .delete(&url)
            .build()
            .map_err(crate::error::HttpError::from)?;
        let response = self.inner.execute_request(request).await?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(crate::http::response_to_force_error(response, error_msg).await)
        }
    }

    /// Retrieves organization limits.
    ///
    /// Returns information about the organization's usage and limits for various
    /// resources including API requests, storage, workflow emails, and more.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Authentication fails
    /// - The HTTP request fails
    /// - The response cannot be deserialized
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let limits = client.rest().limits().await?;
    /// let api_limit = &limits.daily_api_requests;
    /// println!("API calls: {}/{}", api_limit.used.unwrap_or(0), api_limit.max);
    /// ```
    pub async fn limits(&self) -> Result<limits::OrgLimits> {
        self.execute_get("/limits", None, "Limits API request failed")
            .await
    }

    /// Executes a SOSL (Salesforce Object Search Language) search.
    ///
    /// SOSL allows you to search across multiple objects and fields simultaneously.
    ///
    /// # Arguments
    ///
    /// * `sosl` - The SOSL search query (e.g., "FIND {Acme} IN ALL FIELDS RETURNING Account(Name)")
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Authentication fails
    /// - The HTTP request fails
    /// - The SOSL query is malformed
    /// - The response cannot be deserialized
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use force::api::rest::search::SearchQueryBuilder;
    ///
    /// // Using builder
    /// let query = SearchQueryBuilder::new()
    ///     .find("Acme")
    ///     .in_all_fields()
    ///     .returning("Account", &["Id", "Name"])
    ///     .returning("Contact", &["Id", "Name"])
    ///     .limit(10)
    ///     .build();
    ///
    /// let results = client.rest().search(&query).await?;
    ///
    /// for record_set in &results.search_records {
    ///     println!("Found {} {} records",
    ///         record_set.records.len(),
    ///         record_set.attributes.type_);
    /// }
    /// ```
    pub async fn search(&self, sosl: &str) -> Result<search::SearchResult> {
        self.execute_get(
            "/search",
            Some(&[("q", sosl)]),
            "SOSL search request failed",
        )
        .await
    }

    /// Retrieves global describe information.
    ///
    /// Returns metadata for all available SObjects in the organization.
    /// This is a lightweight operation that provides basic information
    /// about each object without field-level details.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Authentication fails
    /// - The HTTP request fails
    /// - The response cannot be deserialized
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let global = client.rest().describe_global().await?;
    ///
    /// for sobject in &global.sobjects {
    ///     if sobject.custom {
    ///         println!("Custom object: {} ({})", sobject.name, sobject.label);
    ///     }
    /// }
    /// ```
    pub async fn describe_global(&self) -> Result<describe::GlobalDescribe> {
        self.execute_get("/sobjects", None, "Global describe request failed")
            .await
    }

    /// Retrieves detailed metadata for a specific SObject.
    ///
    /// Returns comprehensive information including all fields, relationships,
    /// record types, and other metadata for the specified object.
    ///
    /// # Arguments
    ///
    /// * `sobject_name` - The API name of the SObject (e.g., "Account", "Contact")
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Authentication fails
    /// - The HTTP request fails
    /// - The SObject does not exist
    /// - The response cannot be deserialized
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let describe = client.rest().describe("Account").await?;
    ///
    /// println!("Object: {} ({})", describe.name, describe.label);
    /// println!("Fields:");
    /// for field in &describe.fields {
    ///     println!("  {} - {:?} ({})", field.name, field.type_, field.label);
    /// }
    /// ```
    pub async fn describe(&self, sobject_name: &str) -> Result<describe::SObjectDescribe> {
        let path = format!("/sobjects/{}/describe", sobject_name);
        self.execute_get(
            &path,
            None,
            &format!("Describe request for {} failed", sobject_name),
        )
        .await
    }
}
#[cfg(test)]
mod tests {
    use crate::auth::{AccessToken, Authenticator, TokenResponse};
    use crate::client::{ForceClient, builder};
    use crate::config::ClientConfigBuilder;
    use crate::error::Result;
    use crate::test_support::{Must, MustMsg};
    use async_trait::async_trait;

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

    async fn create_test_client() -> ForceClient<MockAuthenticator> {
        let auth = MockAuthenticator::new("test_token", "https://test.salesforce.com");
        builder()
            .authenticate(auth)
            .build()
            .await
            .must_msg("failed to create test client")
    }

    #[tokio::test]
    async fn test_rest_handler_construction() {
        let client: ForceClient<MockAuthenticator> = create_test_client().await;
        let _handler = client.rest();
        // If we get here, handler was created successfully
    }

    #[tokio::test]
    async fn test_rest_handler_is_cloneable() {
        let client: ForceClient<MockAuthenticator> = create_test_client().await;
        let handler1 = client.rest();
        let handler2 = handler1.clone();

        // Both should produce the same base URL
        let url1: String = handler1.base_url().await.must();
        let url2: String = handler2.base_url().await.must();
        assert_eq!(url1, url2);
    }

    #[tokio::test]
    async fn test_base_url_construction() {
        let client: ForceClient<MockAuthenticator> = create_test_client().await;
        let handler = client.rest();

        let base_url: String = handler.base_url().await.must();
        assert!(base_url.starts_with("https://test.salesforce.com"));
        assert!(base_url.contains("/services/data/"));
        assert!(base_url.ends_with("v60.0")); // Default API version
    }

    #[tokio::test]
    async fn test_base_url_with_custom_api_version() {
        let auth = MockAuthenticator::new("test_token", "https://custom.salesforce.com");
        let config = ClientConfigBuilder::new().api_version("v59.0").build();
        let client = builder()
            .authenticate(auth)
            .config(config)
            .build()
            .await
            .must();

        let handler = client.rest();
        let base_url = handler.base_url().await.must();

        assert_eq!(
            base_url,
            "https://custom.salesforce.com/services/data/v59.0"
        );
    }

    #[tokio::test]
    async fn test_base_url_with_different_instance() {
        let auth = MockAuthenticator::new("token", "https://na139.salesforce.com");
        let client = builder().authenticate(auth).build().await.must();

        let handler = client.rest();
        let base_url = handler.base_url().await.must();

        assert!(base_url.starts_with("https://na139.salesforce.com"));
    }

    #[tokio::test]
    async fn test_rest_handler_shares_client_config() {
        let auth = MockAuthenticator::new("token", "https://shared.salesforce.com");
        let config = ClientConfigBuilder::new().api_version("v58.0").build();
        let client = builder()
            .authenticate(auth)
            .config(config)
            .build()
            .await
            .must();

        let handler = client.rest();
        let base_url = handler.base_url().await.must();

        // Verify the handler uses the same config as the client
        assert!(base_url.contains("shared.salesforce.com"));
        assert!(base_url.contains("v58.0"));
    }

    #[tokio::test]
    async fn test_multiple_handlers_from_same_client() {
        let client: ForceClient<MockAuthenticator> = create_test_client().await;

        let handler1 = client.rest();
        let handler2 = client.rest();

        // Both should have the same base URL
        let url1: String = handler1.base_url().await.must();
        let url2: String = handler2.base_url().await.must();
        assert_eq!(url1, url2);
    }

    #[tokio::test]
    async fn test_handler_debug_impl() {
        let client: ForceClient<MockAuthenticator> = create_test_client().await;
        let handler = client.rest();

        let debug_str = format!("{:?}", handler);
        assert!(!debug_str.is_empty());
    }
}
