//! Salesforce Files API.
//!
//! Provides capabilities to upload and download ContentVersions, and link them to records
//! via ContentDocumentLinks without loading the entire binary content into memory.

use crate::auth::Authenticator;
use crate::error::{ForceError, Result};
use crate::session::Session;
use futures::StreamExt;
use reqwest::multipart::{Form, Part};
use serde_json::json;
use std::sync::Arc;

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

    async fn read_capped_body_bytes(
        response: reqwest::Response,
        limit_bytes: usize,
    ) -> std::result::Result<bytes::Bytes, reqwest::Error> {
        let mut stream = response.bytes_stream();
        let init_cap = std::cmp::min(limit_bytes, 4096);
        let mut vec = Vec::with_capacity(init_cap);

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result?;
            let remaining = limit_bytes.saturating_sub(vec.len());
            if chunk.len() > remaining {
                vec.extend_from_slice(&chunk[..remaining]);
                break;
            }
            vec.extend_from_slice(&chunk[..]);
        }
        Ok(vec.into())
    }

    async fn read_capped_body(
        response: reqwest::Response,
        limit_bytes: usize,
    ) -> std::result::Result<String, reqwest::Error> {
        let bytes = Self::read_capped_body_bytes(response, limit_bytes).await?;
        let bytes_vec = bytes.to_vec();
        Ok(String::from_utf8(bytes_vec)
            .unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned()))
    }

    /// Uploads a file, creating a new `ContentVersion`.
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
        let token = self.session.token_manager.token().await?;
        let api_version = self.session.config.api_version.as_str();

        // ⚡ Bolt: Use pre-allocated String instead of `format!` for URL construction to avoid intermediate allocation overhead.
        let instance_url = token.instance_url();
        let capacity = instance_url.len() + 16 + api_version.len() + 24;
        let mut url = String::with_capacity(capacity);
        url.push_str(instance_url);
        url.push_str("/services/data/v");
        url.push_str(api_version);
        url.push_str("/sobjects/ContentVersion");

        let entity_content = json!({
            "Title": title,
            "PathOnClient": path_on_client,
        });

        let entity_part = Part::text(entity_content.to_string())
            .mime_str("application/json")
            .map_err(|e| ForceError::InvalidInput(e.to_string()))?;

        let version_data_part = Part::bytes(file_bytes)
            .file_name(path_on_client.to_string())
            .mime_str("application/octet-stream")
            .map_err(|e| ForceError::InvalidInput(e.to_string()))?;

        let form = Form::new()
            .part("entity_content", entity_part)
            .part("VersionData", version_data_part);

        let response = self
            .session
            .http_client
            .post(&url)
            .bearer_auth(token.as_str())
            .multipart(form)
            .send()
            .await
            .map_err(|e| ForceError::Http(crate::error::HttpError::RequestFailed(e)))?;

        let status = response.status();
        let body = Self::read_capped_body(response, 100 * 1024 * 1024)
            .await
            .map_err(|e| ForceError::Http(crate::error::HttpError::RequestFailed(e)))?;

        if !status.is_success() && status.as_u16() != 201 {
            return Err(ForceError::InvalidInput("Failed to upload ContentVersion".into()));
        }

        let result: serde_json::Value = serde_json::from_str(&body)
            .map_err(|e| ForceError::Serialization(crate::error::SerializationError::Json(e)))?;

        if result["success"].as_bool().unwrap_or(false) {
            Ok(result["id"].as_str().unwrap_or_default().to_string())
        } else {
            Err(ForceError::InvalidInput(
                "Failed to upload ContentVersion".into(),
            ))
        }
    }

    /// Downloads the binary data of a `ContentVersion`.
    ///
    /// # Arguments
    /// * `content_version_id` - The ID of the ContentVersion.
    ///
    /// # Returns
    /// The binary content as a `Vec<u8>`.
    pub async fn download(&self, content_version_id: &str) -> Result<Vec<u8>> {
        let token = self.session.token_manager.token().await?;
        let api_version = self.session.config.api_version.as_str();

        // ⚡ Bolt: Use pre-allocated String instead of `format!` for URL construction to avoid intermediate allocation overhead.
        let instance_url = token.instance_url();
        let capacity = instance_url.len() + 16 + api_version.len() + 25 + content_version_id.len() + 12;
        let mut url = String::with_capacity(capacity);
        url.push_str(instance_url);
        url.push_str("/services/data/v");
        url.push_str(api_version);
        url.push_str("/sobjects/ContentVersion/");
        url.push_str(content_version_id);
        url.push_str("/VersionData");

        let response = self
            .session
            .http_client
            .get(&url)
            .bearer_auth(token.as_str())
            .send()
            .await
            .map_err(|e| ForceError::Http(crate::error::HttpError::RequestFailed(e)))?;

        let status = response.status();
        let bytes = Self::read_capped_body_bytes(response, 100 * 1024 * 1024)
            .await
            .map_err(|e| ForceError::Http(crate::error::HttpError::RequestFailed(e)))?;

        if !status.is_success() && status.as_u16() != 200 {
            return Err(ForceError::InvalidInput("Failed to download ContentVersion".into()));
        }

        Ok(bytes.to_vec())
    }

    /// Links a ContentDocument to an entity (e.g. Account, Contact).
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
        let token = self.session.token_manager.token().await?;
        let api_version = self.session.config.api_version.as_str();

        // ⚡ Bolt: Use pre-allocated String instead of `format!` for URL construction to avoid intermediate allocation overhead.
        let instance_url = token.instance_url();
        let capacity = instance_url.len() + 16 + api_version.len() + 29;
        let mut url = String::with_capacity(capacity);
        url.push_str(instance_url);
        url.push_str("/services/data/v");
        url.push_str(api_version);
        url.push_str("/sobjects/ContentDocumentLink");

        let payload = json!({
            "ContentDocumentId": content_document_id,
            "LinkedEntityId": linked_entity_id,
            "ShareType": "V"
        });

        let response = self
            .session
            .http_client
            .post(&url)
            .bearer_auth(token.as_str())
            .json(&payload)
            .send()
            .await
            .map_err(|e| ForceError::Http(crate::error::HttpError::RequestFailed(e)))?;

        let status = response.status();
        let body = Self::read_capped_body(response, 10 * 1024 * 1024)
            .await
            .map_err(|e| ForceError::Http(crate::error::HttpError::RequestFailed(e)))?;

        if !status.is_success() && status.as_u16() != 201 {
            return Err(ForceError::InvalidInput("Failed to insert ContentDocumentLink".into()));
        }

        let result: serde_json::Value = serde_json::from_str(&body)
            .map_err(|e| ForceError::Serialization(crate::error::SerializationError::Json(e)))?;

        if result["success"].as_bool().unwrap_or(false) {
            Ok(result["id"].as_str().unwrap_or_default().to_string())
        } else {
            Err(ForceError::InvalidInput(
                "Failed to insert ContentDocumentLink".into(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::builder;
    use crate::test_utils::mock_auth::MockAuthenticator;
use crate::test_utils::must::{Must, MustMsg};
    use crate::test_utils::mock_auth::MockAuthenticator;
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_upload_file() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/sobjects/ContentVersion"))
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
                "/services/data/v60.0/sobjects/ContentVersion/068000000000001AAA/VersionData",
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
            .and(path("/services/data/v60.0/sobjects/ContentDocumentLink"))
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
