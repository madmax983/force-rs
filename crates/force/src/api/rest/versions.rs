//! Versions resource (`GET /services/data/`).
//!
//! Lists the API versions that the org supports. From v68.0 (Winter '27) the
//! list also has a `latest` alias entry.

use crate::error::Result;
use crate::types::ApiVersion;
use serde::{Deserialize, Serialize};

/// One entry of the Versions resource.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiVersionInfo {
    /// Release label, for example `"Winter '27"` or `"Latest Release"`.
    pub label: String,
    /// Resource URL, for example `"/services/data/v68.0"`.
    pub url: String,
    /// Version number, for example `"68.0"`.
    pub version: String,
}

impl ApiVersionInfo {
    /// Returns true if this entry is the `latest` alias.
    #[must_use]
    pub fn is_latest_alias(&self) -> bool {
        self.url
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .is_some_and(ApiVersion::is_latest_alias)
    }

    /// Returns the parsed version. Returns `None` if `version` is not numeric.
    #[must_use]
    pub fn api_version(&self) -> Option<ApiVersion> {
        format!("v{}", self.version).parse().ok()
    }
}

/// Returns the highest numeric version in `versions`. Skips the `latest` alias.
#[must_use]
fn highest_numeric_version(versions: &[ApiVersionInfo]) -> Option<ApiVersion> {
    versions
        .iter()
        .filter(|v| !v.is_latest_alias())
        .filter_map(ApiVersionInfo::api_version)
        .max()
}

impl<A: crate::auth::Authenticator> super::RestHandler<A> {
    /// Lists the API versions that the org supports.
    ///
    /// `GET /services/data/`. From v68.0 the list has a `latest` alias entry.
    /// Use [`ApiVersionInfo::is_latest_alias`] to find it.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response cannot be decoded.
    pub async fn versions(&self) -> Result<Vec<ApiVersionInfo>> {
        let instance_url = self.inner.instance_url().await?;
        let url = format!("{}/services/data/", instance_url.trim_end_matches('/'));
        let request = self
            .inner
            .get(&url)
            .build()
            .map_err(crate::error::HttpError::from)?;
        self.inner
            .send_request_and_decode(request, "Versions request failed")
            .await
    }

    /// Returns the highest numeric API version that the org supports.
    ///
    /// Skips the `latest` alias entry. Returns `None` if the list has no
    /// numeric version.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response cannot be decoded.
    pub async fn highest_version(&self) -> Result<Option<ApiVersion>> {
        Ok(highest_numeric_version(&self.versions().await?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::builder;
    use crate::test_utils::mock_auth::MockAuthenticator;
    use crate::test_utils::must::Must;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn v68_body() -> serde_json::Value {
        serde_json::json!([
            {"label": "Summer '26", "url": "/services/data/v67.0", "version": "67.0"},
            {"label": "Winter '27", "url": "/services/data/v68.0", "version": "68.0"},
            {"label": "Latest Release", "url": "/services/data/latest", "version": "68.0"}
        ])
    }

    #[test]
    fn test_latest_alias_entry_detected() {
        let list: Vec<ApiVersionInfo> = serde_json::from_value(v68_body()).must();
        assert!(!list[0].is_latest_alias());
        assert!(list[2].is_latest_alias());
    }

    #[test]
    fn test_placeholder_version_does_not_parse() {
        // The guide prints "XX.0" for the alias entry.
        let entry = ApiVersionInfo {
            label: "Latest Release".into(),
            url: "/services/data/latest/".into(),
            version: "XX.0".into(),
        };
        assert!(entry.is_latest_alias());
        assert_eq!(entry.api_version(), None);
    }

    #[test]
    fn test_highest_version_skips_alias() {
        let mut list: Vec<ApiVersionInfo> = serde_json::from_value(v68_body()).must();
        // Alias says 99.0; it must not win.
        list[2].version = "99.0".into();
        assert_eq!(highest_numeric_version(&list), Some(ApiVersion::V68));
    }

    #[test]
    fn test_highest_version_empty() {
        assert_eq!(highest_numeric_version(&[]), None);
    }

    #[test]
    fn test_highest_version_unsorted_and_alias_only() {
        let list: Vec<ApiVersionInfo> = serde_json::from_value(serde_json::json!([
            {"label": "Winter '27", "url": "/services/data/v68.0", "version": "68.0"},
            {"label": "Summer '26", "url": "/services/data/v67.0", "version": "67.0"},
            {"label": "Spring '26", "url": "/services/data/v66.0", "version": "66.0"}
        ]))
        .must();
        assert_eq!(highest_numeric_version(&list), Some(ApiVersion::V68));

        let alias_only: Vec<ApiVersionInfo> = serde_json::from_value(serde_json::json!([
            {"label": "Latest Release", "url": "/services/data/latest", "version": "68.0"}
        ]))
        .must();
        assert_eq!(highest_numeric_version(&alias_only), None);
    }

    #[tokio::test]
    async fn test_versions_and_latest_version() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/services/data/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(v68_body()))
            .expect(2)
            .mount(&server)
            .await;
        let client = builder()
            .authenticate(MockAuthenticator::new("t", &server.uri()))
            .build()
            .await
            .must();

        let list = client.rest().versions().await.must();
        assert_eq!(list.len(), 3);
        assert_eq!(list[1].api_version(), Some(ApiVersion::V68));

        let highest = client.rest().highest_version().await.must();
        assert_eq!(highest, Some(ApiVersion::V68));
    }

    #[tokio::test]
    async fn test_versions_error_status() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/services/data/"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;
        let client = builder()
            .authenticate(MockAuthenticator::new("t", &server.uri()))
            .build()
            .await
            .must();

        assert!(client.rest().versions().await.is_err());
    }
}
