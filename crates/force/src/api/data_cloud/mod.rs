//! Salesforce Data Cloud REST Connect API handler.
//!
//! The Data Cloud API (`/services/data/vXX.0/ssot/`) provides access to the
//! Salesforce Data Cloud (formerly CDP) platform, including SQL queries,
//! calculated insights, and data ingestion.
//!
//! Data Cloud uses a separate tenant endpoint and requires a two-step token
//! exchange. The [`DataCloudHandler`] manages this transparently via the
//! [`DataCloudAuthenticator`](crate::auth::DataCloudAuthenticator) decorator.
//!
//! # Feature Flag
//!
//! This module requires the `data_cloud` feature flag:
//! ```toml
//! [dependencies]
//! force = { version = "...", features = ["data_cloud"] }
//! ```
//!
//! # Usage
//!
//! ```ignore
//! use force::auth::DataCloudConfig;
//!
//! let client = builder()
//!     .authenticate(auth)
//!     .with_data_cloud(DataCloudConfig::default())
//!     .build()
//!     .await?;
//!
//! let dc = client.data_cloud()?;
//! let result = dc.query_sql("SELECT * FROM UnifiedProfile__dlm LIMIT 10").await?;
//! ```

pub mod query;
pub mod types;

pub use query::SqlQueryResponse;
pub use types::{DataCloudRecord, SqlQueryRequest};

use crate::auth::data_cloud::DataCloudAuthenticator;
use crate::error::Result;
use std::sync::Arc;

/// Data Cloud API handler.
///
/// Provides access to the Salesforce Data Cloud REST Connect API endpoints.
/// Internally wraps a [`Session`](crate::session::Session) authenticated with
/// a [`DataCloudAuthenticator`], which manages the token exchange transparently.
///
/// Obtain a handler from [`ForceClient::data_cloud`](crate::client::ForceClient::data_cloud).
#[derive(Debug)]
pub struct DataCloudHandler<A: crate::auth::Authenticator> {
    /// Reference to the Data Cloud session state.
    inner: Arc<crate::session::Session<DataCloudAuthenticator<A>>>,
}

impl<A: crate::auth::Authenticator> Clone for DataCloudHandler<A> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<A: crate::auth::Authenticator> DataCloudHandler<A> {
    /// Creates a new Data Cloud handler wrapping the given session.
    #[must_use]
    pub(crate) fn new(inner: Arc<crate::session::Session<DataCloudAuthenticator<A>>>) -> Self {
        Self { inner }
    }

    /// Resolves a Data Cloud API path to a full URL.
    ///
    /// Constructs: `{dc_instance_url}/services/data/{version}/ssot/{path}`
    pub(crate) async fn resolve_dc_url(&self, path: &str) -> Result<String> {
        let clean = path.trim_start_matches('/');
        if clean.is_empty() {
            self.inner.resolve_url("ssot").await
        } else {
            self.inner.resolve_url(&format!("ssot/{clean}")).await
        }
    }

    /// Helper: POST to a Data Cloud path and deserialize the JSON response.
    pub(crate) async fn post<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: &(impl serde::Serialize + Sync),
        error_msg: &str,
    ) -> Result<T> {
        let url = self.resolve_dc_url(path).await?;
        let request = self
            .inner
            .post(&url)
            .json(body)
            .build()
            .map_err(crate::error::HttpError::from)?;
        self.inner.send_request_and_decode(request, error_msg).await
    }

    /// Helper: GET a Data Cloud path and deserialize the JSON response.
    pub(crate) async fn get<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        query: Option<&[(&str, &str)]>,
        error_msg: &str,
    ) -> Result<T> {
        let url = self.resolve_dc_url(path).await?;
        let mut req = self.inner.get(&url);
        if let Some(params) = query {
            req = req.query(params);
        }
        let request = req.build().map_err(crate::error::HttpError::from)?;
        self.inner.send_request_and_decode(request, error_msg).await
    }

    /// Helper: DELETE a Data Cloud path; expect 204 No Content.
    #[allow(dead_code)]
    pub(crate) async fn delete_empty(&self, path: &str, error_msg: &str) -> Result<()> {
        let url = self.resolve_dc_url(path).await?;
        let request = self
            .inner
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
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use crate::auth::DataCloudConfig;
    use crate::client::{ForceClient, builder};
    use crate::test_support::{MockAuthenticator, Must, MustMsg};

    async fn test_dc_client() -> ForceClient<MockAuthenticator> {
        let auth = MockAuthenticator::new("test_token", "https://test.salesforce.com");
        builder()
            .authenticate(auth)
            .with_data_cloud(DataCloudConfig::default())
            .build()
            .await
            .must_msg("failed to create test client with DC")
    }

    async fn test_client_no_dc() -> ForceClient<MockAuthenticator> {
        let auth = MockAuthenticator::new("test_token", "https://test.salesforce.com");
        builder()
            .authenticate(auth)
            .build()
            .await
            .must_msg("failed to create test client without DC")
    }

    #[tokio::test]
    async fn test_data_cloud_handler_construction() {
        let client = test_dc_client().await;
        let _handler = client.data_cloud().must();
    }

    #[tokio::test]
    async fn test_data_cloud_handler_is_cloneable() {
        let client = test_dc_client().await;
        let h1 = client.data_cloud().must();
        let _h2 = h1.clone();
    }

    #[tokio::test]
    async fn test_data_cloud_not_configured_returns_error() {
        let client = test_client_no_dc().await;
        let result = client.data_cloud();
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("Data Cloud not configured"),
            "Error should mention DC not configured, got: {err}"
        );
    }

    #[tokio::test]
    async fn test_data_cloud_handler_debug() {
        let client = test_dc_client().await;
        let handler = client.data_cloud().must();
        let debug = format!("{handler:?}");
        assert!(!debug.is_empty());
    }

    #[tokio::test]
    async fn test_client_clone_preserves_dc_session() {
        let client = test_dc_client().await;
        let cloned = client.clone();
        assert!(cloned.data_cloud().is_ok());
    }

    #[tokio::test]
    async fn test_builder_without_dc_still_works() {
        let client = test_client_no_dc().await;
        // Platform APIs should still work
        assert_eq!(client.config().api_version, "v60.0");
    }

    #[tokio::test]
    async fn test_builder_with_dc_api_version_override() {
        let auth = MockAuthenticator::new("test_token", "https://test.salesforce.com");
        let client = builder()
            .authenticate(auth)
            .with_data_cloud(DataCloudConfig {
                api_version: Some("v64.0".into()),
                ..Default::default()
            })
            .build()
            .await
            .must();

        // Platform API version unchanged
        assert_eq!(client.config().api_version, "v60.0");
        // DC handler exists
        assert!(client.data_cloud().is_ok());
    }
}
