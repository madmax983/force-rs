//! Salesforce Files API.
//!
//! Provides capabilities to upload and download ContentVersions, and link them to records
//! via ContentDocumentLinks without loading the entire binary content into memory.

use crate::auth::Authenticator;
use crate::error::{ForceError, HttpError, Result};
use crate::session::Session;
use reqwest::multipart::{Form, Part};
use serde_json::json;
use std::sync::Arc;

/// Maximum body size (100 MiB) accepted for a `ContentVersion` download/upload.
const MAX_CONTENT_BODY: usize = 100 * 1024 * 1024;

/// Maximum body size (10 MiB) accepted for a `ContentDocumentLink` JSON response.
const MAX_LINK_BODY: usize = 10 * 1024 * 1024;

/// A handler for interacting with Salesforce Files.
#[derive(Debug, Clone)]
pub struct FilesHandler<A: Authenticator> {
    session: Arc<Session<A>>,
}

impl<A: Authenticator> FilesHandler<A> {
    /// Creates a new Files API handler.
    #[must_use]
    pub fn new(session: Arc<Session<A>>) -> Self {
        Self { session }
    }

    /// Uploads a file, creating a new `ContentVersion`.
    ///
    /// The request is routed through the shared HTTP executor, so it benefits
    /// from automatic 401 token refresh, 429/`Retry-After` handling, and 503
    /// backoff. Because the multipart body cannot be cloned for a retry, the
    /// request is rebuilt from the retained title/path/bytes on each attempt.
    ///
    /// # Arguments
    /// * `title` - The title of the document.
    /// * `path_on_client` - The original filename.
    /// * `file_bytes` - The contents of the file.
    ///
    /// # Returns
    /// The ID of the inserted `ContentVersion`.
    pub async fn upload(
        &self,
        title: &str,
        path_on_client: &str,
        file_bytes: Vec<u8>,
    ) -> Result<String> {
        /// ⚡ Bolt: Use `bytes::Bytes` and `Part::stream()` to provide O(1) cloning for large `Vec<u8>` payloads on request retries instead of `Part::bytes()`.
        fn _bolt_optimization() {}

        let url = self.session.resolve_url("sobjects/ContentVersion").await?;

        let session = Arc::clone(&self.session);
        let title = title.to_string();
        let path_on_client = path_on_client.to_string();
        let file_bytes = bytes::Bytes::from(file_bytes);

        // Rebuild the multipart form (and its body) on every attempt: streaming
        // multipart bodies cannot be cloned, so the executor calls this factory
        // fresh for each retry / post-401 retry.
        let make_request = move || -> Result<reqwest::Request> {
            let entity_content = json!({
                "Title": title,
                "PathOnClient": path_on_client,
            });

            let entity_part = Part::text(entity_content.to_string())
                .mime_str("application/json")
                .map_err(|e| ForceError::InvalidInput(e.to_string()))?;

            let version_data_part = Part::stream(file_bytes.clone())
                .file_name(path_on_client.clone())
                .mime_str("application/octet-stream")
                .map_err(|e| ForceError::InvalidInput(e.to_string()))?;

            let form = Form::new()
                .part("entity_content", entity_part)
                .part("VersionData", version_data_part);

            session
                .post(&url)
                .multipart(form)
                .build()
                .map_err(|e| ForceError::Http(HttpError::RequestFailed(e)))
        };

        let response = self.session.execute_request_factory(make_request).await?;
        let status = response.status();
        let bytes = crate::http::error::read_capped_body_bytes(response, MAX_CONTENT_BODY).await?;

        if !status.is_success() {
            return Err(ForceError::InvalidInput(
                "Failed to upload ContentVersion".into(),
            ));
        }

        Self::extract_created_id(&bytes, "Failed to upload ContentVersion")
    }

    /// Downloads the binary data of a `ContentVersion`.
    ///
    /// Routed through the shared HTTP executor (401 refresh + retry / rate-limit
    /// / backoff middleware).
    ///
    /// # Arguments
    /// * `content_version_id` - The ID of the ContentVersion.
    ///
    /// # Returns
    /// The binary content as a `Vec<u8>`.
    pub async fn download(&self, content_version_id: &str) -> Result<Vec<u8>> {
        let path = format!("sobjects/ContentVersion/{content_version_id}/VersionData");
        let url = self.session.resolve_url(&path).await?;

        let request = self
            .session
            .get(&url)
            .build()
            .map_err(|e| ForceError::Http(HttpError::RequestFailed(e)))?;

        let response = self.session.execute_request(request).await?;
        let status = response.status();
        let bytes = crate::http::error::read_capped_body_bytes(response, MAX_CONTENT_BODY).await?;

        if !status.is_success() {
            return Err(ForceError::InvalidInput(
                "Failed to download ContentVersion".into(),
            ));
        }

        Ok(bytes)
    }

    /// Links a ContentDocument to an entity (e.g. Account, Contact).
    ///
    /// Routed through the shared HTTP executor (401 refresh + retry / rate-limit
    /// / backoff middleware).
    ///
    /// # Arguments
    /// * `content_document_id` - The ID of the ContentDocument (not the ContentVersion).
    /// * `linked_entity_id` - The ID of the target Salesforce record.
    ///
    /// # Returns
    /// The ID of the inserted `ContentDocumentLink`.
    pub async fn link_to_record(
        &self,
        content_document_id: &str,
        linked_entity_id: &str,
    ) -> Result<String> {
        let url = self
            .session
            .resolve_url("sobjects/ContentDocumentLink")
            .await?;

        let payload = json!({
            "ContentDocumentId": content_document_id,
            "LinkedEntityId": linked_entity_id,
            "ShareType": "V"
        });

        let request = self
            .session
            .post(&url)
            .json(&payload)
            .build()
            .map_err(|e| ForceError::Http(HttpError::RequestFailed(e)))?;

        let response = self.session.execute_request(request).await?;
        let status = response.status();
        let bytes = crate::http::error::read_capped_body_bytes(response, MAX_LINK_BODY).await?;

        if !status.is_success() {
            return Err(ForceError::InvalidInput(
                "Failed to insert ContentDocumentLink".into(),
            ));
        }

        Self::extract_created_id(&bytes, "Failed to insert ContentDocumentLink")
    }

    /// Parses a Salesforce sObject-create response body, returning the new record
    /// `id` when `success` is `true`, or `error_message` otherwise.
    fn extract_created_id(body: &[u8], error_message: &'static str) -> Result<String> {
        let result: serde_json::Value = serde_json::from_slice(body)
            .map_err(|e| ForceError::Serialization(crate::error::SerializationError::Json(e)))?;

        if result["success"].as_bool().unwrap_or(false) {
            Ok(result["id"].as_str().unwrap_or_default().to_string())
        } else {
            Err(ForceError::InvalidInput(error_message.into()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::AccessToken;
    use crate::client::builder;
    use crate::test_utils::mock_auth::MockAuthenticator;
    use crate::test_utils::must::{Must, MustMsg};
    use async_trait::async_trait;
    use serde_json::json;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// Test authenticator whose initial `authenticate()` yields an "expired"
    /// (server-rejected) token and whose `refresh()` yields a distinct fresh
    /// token, counting how many times a refresh occurs. This lets a handler-level
    /// test prove that a 401 triggers a token refresh + retry through the shared
    /// executor middleware.
    #[derive(Debug)]
    struct RefreshingAuthenticator {
        instance_url: String,
        refresh_calls: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl Authenticator for RefreshingAuthenticator {
        async fn authenticate(&self) -> crate::error::Result<AccessToken> {
            // Valid (not soft-expired) so the token manager hands it out as-is;
            // the *server* is what rejects it with a 401.
            Ok(AccessToken::new(
                "expired_token".to_string(),
                self.instance_url.clone(),
                Some(chrono::Utc::now() + chrono::Duration::hours(1)),
            ))
        }

        async fn refresh(&self) -> crate::error::Result<AccessToken> {
            self.refresh_calls.fetch_add(1, Ordering::SeqCst);
            Ok(AccessToken::new(
                "fresh_token".to_string(),
                self.instance_url.clone(),
                Some(chrono::Utc::now() + chrono::Duration::hours(2)),
            ))
        }
    }

    #[tokio::test]
    async fn test_download_refreshes_on_401_and_retries() {
        let mock_server = MockServer::start().await;

        // First attempt with the stale token is rejected with 401.
        Mock::given(method("GET"))
            .and(path(
                "/services/data/v67.0/sobjects/ContentVersion/068000000000001AAA/VersionData",
            ))
            .and(header("Authorization", "Bearer expired_token"))
            .respond_with(ResponseTemplate::new(401).set_body_json(json!([{
                "message": "Session expired or invalid",
                "errorCode": "INVALID_SESSION_ID"
            }])))
            .up_to_n_times(1)
            .mount(&mock_server)
            .await;

        // Second attempt with the refreshed token succeeds with the bytes.
        let expected_bytes = vec![9, 8, 7, 6, 5];
        Mock::given(method("GET"))
            .and(path(
                "/services/data/v67.0/sobjects/ContentVersion/068000000000001AAA/VersionData",
            ))
            .and(header("Authorization", "Bearer fresh_token"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(expected_bytes.clone()))
            .mount(&mock_server)
            .await;

        let refresh_calls = Arc::new(AtomicUsize::new(0));
        let auth = RefreshingAuthenticator {
            instance_url: mock_server.uri(),
            refresh_calls: Arc::clone(&refresh_calls),
        };
        let client = builder().authenticate(auth).build().await.must();

        let bytes = client
            .files()
            .download("068000000000001AAA")
            .await
            .must_msg("download should refresh on 401 and retry");

        assert_eq!(bytes, expected_bytes);
        assert_eq!(
            refresh_calls.load(Ordering::SeqCst),
            1,
            "expected exactly one token refresh triggered by the 401"
        );
    }

    #[tokio::test]
    async fn test_upload_refreshes_on_401_and_retries() {
        let mock_server = MockServer::start().await;

        // First multipart POST with the stale token is rejected with 401. This
        // exercises the non-clonable factory path: the executor must rebuild the
        // multipart body from scratch (via `make_request`) for the retry.
        Mock::given(method("POST"))
            .and(path("/services/data/v67.0/sobjects/ContentVersion"))
            .and(header("Authorization", "Bearer expired_token"))
            .respond_with(ResponseTemplate::new(401).set_body_json(json!([{
                "message": "Session expired or invalid",
                "errorCode": "INVALID_SESSION_ID"
            }])))
            .up_to_n_times(1)
            .mount(&mock_server)
            .await;

        // Second POST with the refreshed token succeeds, returning the new id.
        Mock::given(method("POST"))
            .and(path("/services/data/v67.0/sobjects/ContentVersion"))
            .and(header("Authorization", "Bearer fresh_token"))
            .respond_with(ResponseTemplate::new(201).set_body_json(json!({
                "id": "068000000000001AAA",
                "success": true
            })))
            .mount(&mock_server)
            .await;

        let refresh_calls = Arc::new(AtomicUsize::new(0));
        let auth = RefreshingAuthenticator {
            instance_url: mock_server.uri(),
            refresh_calls: Arc::clone(&refresh_calls),
        };
        let client = builder().authenticate(auth).build().await.must();

        let id = client
            .files()
            .upload("Test Title", "test.pdf", vec![1, 2, 3])
            .await
            .must_msg("upload should refresh on 401 and retry");

        assert_eq!(id, "068000000000001AAA");
        assert_eq!(
            refresh_calls.load(Ordering::SeqCst),
            1,
            "expected exactly one token refresh triggered by the 401"
        );
    }

    #[tokio::test]
    async fn test_upload_file() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        Mock::given(method("POST"))
            .and(path("/services/data/v67.0/sobjects/ContentVersion"))
            .respond_with(ResponseTemplate::new(201).set_body_json(json!({
                "id": "068000000000001AAA",
                "success": true,
                "errors": []
            })))
            .mount(&mock_server)
            .await;

        let files = FilesHandler::new(client.session());
        let id = files
            .upload("Test Title", "test.pdf", vec![1, 2, 3])
            .await
            .must_msg("Failed to upload file");

        assert_eq!(id, "068000000000001AAA");
    }

    #[tokio::test]
    async fn test_download_file() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        let expected_bytes = vec![1, 2, 3, 4, 5];

        Mock::given(method("GET"))
            .and(path(
                "/services/data/v67.0/sobjects/ContentVersion/068000000000001AAA/VersionData",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(expected_bytes.clone()))
            .mount(&mock_server)
            .await;

        let files = FilesHandler::new(client.session());
        let bytes = files
            .download("068000000000001AAA")
            .await
            .must_msg("Failed to download file");

        assert_eq!(bytes, expected_bytes);
    }

    #[tokio::test]
    async fn test_link_to_record() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        Mock::given(method("POST"))
            .and(path("/services/data/v67.0/sobjects/ContentDocumentLink"))
            .respond_with(ResponseTemplate::new(201).set_body_json(json!({
                "id": "06A000000000001AAA",
                "success": true,
                "errors": []
            })))
            .mount(&mock_server)
            .await;

        let files = FilesHandler::new(client.session());
        let link_id = files
            .link_to_record("069000000000001AAA", "001000000000001AAA")
            .await
            .must_msg("Failed to link file to record");

        assert_eq!(link_id, "06A000000000001AAA");
    }
}
