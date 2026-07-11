//! Salesforce UI API handler.
//!
//! The UI API (`/services/data/vXX.0/ui-api/`) provides layout-aware record
//! data, object metadata, list views, actions, lookups, and favorites. Unlike
//! the REST API which operates on raw SObject data, the UI API returns
//! presentation-ready structures including display values, layout sections,
//! and user-context-aware field visibility.
//!
//! # Feature Flag
//!
//! This module requires the `ui` feature flag:
//! ```toml
//! [dependencies]
//! force = { version = "...", features = ["ui"] }
//! ```
//!
//! # Usage
//!
//! ```ignore
//! let client = builder().authenticate(auth).build().await?;
//! let ui = client.ui();
//!
//! // Get layout-aware record data
//! let record_ui = ui.record_ui(&["001000000000001AAA"], None, None).await?;
//!
//! // Get object metadata
//! let info = ui.object_info("Account").await?;
//! ```

#![allow(clippy::doc_markdown)]

pub(crate) mod actions;
pub(crate) mod favorites;
pub(crate) mod layouts;
pub(crate) mod list_views;
pub(crate) mod lookups;
pub(crate) mod object_info;
pub(crate) mod records;
pub(crate) mod types;

// Re-export shared types at module level
pub use actions::{ActionRepresentation, RecordActionRepresentation};
pub use favorites::{FavoriteInput, FavoriteRepresentation, FavoritesRepresentation};
pub use layouts::{LayoutItem, LayoutRow, LayoutSection, RecordLayoutRepresentation};
pub use list_views::{
    ListInfoRepresentation, ListRecordsRepresentation, ListUiRepresentation, ListViewSummary,
    ListViewSummaryCollection,
};
pub use lookups::{LookupResult, LookupResultsRepresentation};
pub use object_info::{
    BatchObjectInfoRepresentation, FieldInfoRepresentation, ObjectInfoRepresentation,
    ReferenceToInfoRepresentation,
};
pub use records::{
    BatchResultRepresentation, CreateRecordInput, RecordDefaultsRepresentation,
    RecordRepresentation, RecordUiRepresentation, UpdateRecordInput,
};
pub use types::{FieldValueRepresentation, LayoutType, Mode};

use crate::error::Result;
use std::sync::Arc;

/// UI API handler for Salesforce layout-aware operations.
///
/// The handler provides access to the UI API, which returns presentation-ready
/// record data, object metadata, list views, actions, lookups, and favorites.
///
/// Obtain a handler from [`ForceClient::ui`](crate::client::ForceClient::ui).
#[derive(Debug)]
pub struct UiHandler<A: crate::auth::Authenticator> {
    /// Reference to the shared session state.
    inner: Arc<crate::session::Session<A>>,
}

impl<A: crate::auth::Authenticator> Clone for UiHandler<A> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<A: crate::auth::Authenticator> UiHandler<A> {
    /// Creates a new UI handler wrapping the given session.
    #[must_use]
    pub(crate) fn new(inner: Arc<crate::session::Session<A>>) -> Self {
        Self { inner }
    }

    /// Resolves a UI API path to a full URL.
    ///
    /// Constructs: `{instance_url}/services/data/{version}/ui-api/{path}`
    pub(crate) async fn resolve_ui_url(&self, path: &str) -> Result<String> {
        let clean = path.trim_start_matches('/');
        if clean.is_empty() {
            self.inner.resolve_url("ui-api").await
        } else {
            self.inner.resolve_url(&format!("ui-api/{clean}")).await
        }
    }

    /// Helper: GET a UI API path and deserialize the JSON response.
    pub(crate) async fn get<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        query: Option<&[(&str, &str)]>,
        error_msg: &str,
    ) -> Result<T> {
        let url = self.resolve_ui_url(path).await?;
        let mut req = self.inner.get(&url);
        if let Some(params) = query {
            req = req.query(params);
        }
        let request = req.build().map_err(crate::error::HttpError::from)?;
        self.inner.send_request_and_decode(request, error_msg).await
    }

    /// Helper: POST to a UI API path and deserialize the JSON response.
    pub(crate) async fn post<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: &(impl serde::Serialize + Sync),
        error_msg: &str,
    ) -> Result<T> {
        let url = self.resolve_ui_url(path).await?;
        let request = self
            .inner
            .post(&url)
            .json(body)
            .build()
            .map_err(crate::error::HttpError::from)?;
        self.inner.send_request_and_decode(request, error_msg).await
    }

    /// Helper: PATCH to a UI API path and deserialize the JSON response.
    pub(crate) async fn patch<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: &(impl serde::Serialize + Sync),
        error_msg: &str,
    ) -> Result<T> {
        let url = self.resolve_ui_url(path).await?;
        let request = self
            .inner
            .patch(&url)
            .json(body)
            .build()
            .map_err(crate::error::HttpError::from)?;
        self.inner.send_request_and_decode(request, error_msg).await
    }

    /// Helper: DELETE a UI API path; expect 204 No Content.
    pub(crate) async fn delete_empty(&self, path: &str, error_msg: &str) -> Result<()> {
        let url = self.resolve_ui_url(path).await?;
        let request = self
            .inner
            .delete(&url)
            .build()
            .map_err(crate::error::HttpError::from)?;
        self.inner
            .execute_and_check_success(request, error_msg)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use crate::client::{ForceClient, builder};
    use crate::test_utils::mock_auth::MockAuthenticator;
    use crate::test_utils::must::{Must, MustMsg};

    async fn test_client() -> ForceClient<MockAuthenticator> {
        let auth = MockAuthenticator::new("test_token", "https://test.salesforce.com");
        builder()
            .authenticate(auth)
            .build()
            .await
            .must_msg("failed to create test client")
    }

    #[tokio::test]
    async fn test_ui_handler_construction() {
        let client = test_client().await;
        let _handler = client.ui();
    }

    #[tokio::test]
    async fn test_ui_handler_is_cloneable() {
        let client = test_client().await;
        let h1 = client.ui();
        let h2 = h1.clone();
        // Both should resolve the same base URL
        let url1 = h1.resolve_ui_url("").await.must();
        let url2 = h2.resolve_ui_url("").await.must();
        assert_eq!(url1, url2);
    }

    #[tokio::test]
    async fn test_resolve_ui_url_base() {
        let client = test_client().await;
        let handler = client.ui();
        let url = handler.resolve_ui_url("").await.must();
        assert!(url.contains("/services/data/"));
        assert!(url.ends_with("ui-api"));
    }

    #[tokio::test]
    async fn test_resolve_ui_url_with_path() {
        let client = test_client().await;
        let handler = client.ui();
        let url = handler
            .resolve_ui_url("records/001000000000001AAA")
            .await
            .must();
        assert!(url.contains("ui-api/records/001000000000001AAA"));
    }

    #[tokio::test]
    async fn test_resolve_ui_url_leading_slash() {
        let client = test_client().await;
        let handler = client.ui();
        // Leading slash should be stripped
        let url1 = handler.resolve_ui_url("object-info/Account").await.must();
        let url2 = handler.resolve_ui_url("/object-info/Account").await.must();
        assert_eq!(url1, url2);
    }

    #[tokio::test]
    async fn test_multiple_handlers_share_session() {
        let client = test_client().await;
        let h1 = client.ui();
        let h2 = client.ui();
        let url1 = h1.resolve_ui_url("favorites").await.must();
        let url2 = h2.resolve_ui_url("favorites").await.must();
        assert_eq!(url1, url2);
    }

    #[tokio::test]
    async fn test_ui_handler_debug() {
        let client = test_client().await;
        let handler = client.ui();
        let debug = format!("{handler:?}");
        assert!(!debug.is_empty());
    }

    #[tokio::test]
    async fn test_ui_url_includes_api_version() {
        let client = test_client().await;
        let handler = client.ui();
        let url = handler.resolve_ui_url("records/001test").await.must();
        assert!(url.contains("v67.0"), "URL should contain API version");
    }
}
