//! API Catalog handler: MCP server registry (Winter '27, v68.0+).
//!
//! The API Catalog (`/services/data/vXX.X/api-catalog/`) registers external
//! MCP servers in the org. Agentforce can then use their tools, prompts, and
//! resources. This handler manages server records and their assets. It is not
//! an MCP client.
//!
//! # Feature Flag
//!
//! ```toml
//! [dependencies]
//! force = { version = "...", features = ["api_catalog"] }
//! ```
//!
//! # Usage
//!
//! ```ignore
//! let catalog = client.api_catalog();
//! let input = McpServerCreateInput::external("weather", "https://mcp.example.com/mcp");
//! let created = catalog.create_mcp_server(&input).await?;
//! let assets = catalog.fetch_mcp_server(&created.server.id).await?;
//! ```

pub(crate) mod types;

pub use types::{
    ListMcpServersOptions, McpAssetKind, McpAuthType, McpFetchedAsset, McpServer, McpServerAsset,
    McpServerAssetCollection, McpServerAssetReplaceInput, McpServerAssetReplaceItem,
    McpServerAuthorization, McpServerAuthorizationInput, McpServerCollection, McpServerCreateInput,
    McpServerCreateOutput, McpServerFetchOutput, McpServerStatusFilter, McpServerType,
    McpServerUpdateInput,
};

use crate::error::{ForceError, HttpError, Result};
use serde::de::DeserializeOwned;
use std::sync::Arc;

/// API Catalog handler.
///
/// Obtain a handler from [`ForceClient::api_catalog`](crate::client::ForceClient::api_catalog).
#[derive(Debug)]
pub struct ApiCatalogHandler<A: crate::auth::Authenticator> {
    inner: Arc<crate::session::Session<A>>,
}

impl<A: crate::auth::Authenticator> Clone for ApiCatalogHandler<A> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

/// Checks that `id` is a safe record ID path segment.
fn validate_id(id: &str) -> Result<()> {
    if id.is_empty() || !id.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return Err(ForceError::InvalidInput(format!(
            "MCP server ID must be alphanumeric: {id:?}"
        )));
    }
    Ok(())
}

impl<A: crate::auth::Authenticator> ApiCatalogHandler<A> {
    /// Creates a new handler wrapping the given session.
    #[must_use]
    pub(crate) fn new(inner: Arc<crate::session::Session<A>>) -> Self {
        Self { inner }
    }

    /// Resolves `api-catalog/mcp-servers[/{suffix}]` to a full URL.
    async fn url(&self, suffix: &str) -> Result<String> {
        let path = if suffix.is_empty() {
            "api-catalog/mcp-servers".to_owned()
        } else {
            format!("api-catalog/mcp-servers/{suffix}")
        };
        self.inner.resolve_url(&path).await
    }

    /// Sends `request` and decodes the JSON response.
    async fn send<T: DeserializeOwned>(
        &self,
        request: reqwest::RequestBuilder,
        error_msg: &str,
    ) -> Result<T> {
        let request = request.build().map_err(HttpError::from)?;
        self.inner.send_request_and_decode(request, error_msg).await
    }

    /// Lists MCP servers.
    ///
    /// `GET /api-catalog/mcp-servers`
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response cannot be decoded.
    pub async fn list_mcp_servers(
        &self,
        options: &ListMcpServersOptions,
    ) -> Result<McpServerCollection> {
        let url = self.url("").await?;
        let request = self.inner.get(&url).query(&options.query_params());
        self.send(request, "List MCP servers failed").await
    }

    /// Registers an MCP server and returns its first asset fetch.
    ///
    /// `POST /api-catalog/mcp-servers`
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response cannot be decoded.
    pub async fn create_mcp_server(
        &self,
        input: &McpServerCreateInput,
    ) -> Result<McpServerCreateOutput> {
        let url = self.url("").await?;
        self.send(
            self.inner.post(&url).json(input),
            "Create MCP server failed",
        )
        .await
    }

    /// Gets one MCP server.
    ///
    /// `GET /api-catalog/mcp-servers/{id}`
    ///
    /// # Errors
    ///
    /// Returns an error if `id` is not valid, the request fails, or the
    /// response cannot be decoded.
    pub async fn get_mcp_server(&self, id: &str) -> Result<McpServer> {
        validate_id(id)?;
        let url = self.url(id).await?;
        self.send(self.inner.get(&url), "Get MCP server failed")
            .await
    }

    /// Changes an MCP server.
    ///
    /// `PUT /api-catalog/mcp-servers/{id}`
    ///
    /// # Errors
    ///
    /// Returns an error if `id` is not valid, the request fails, or the
    /// response cannot be decoded.
    pub async fn update_mcp_server(
        &self,
        id: &str,
        input: &McpServerUpdateInput,
    ) -> Result<McpServer> {
        validate_id(id)?;
        let url = self.url(id).await?;
        self.send(self.inner.put(&url).json(input), "Update MCP server failed")
            .await
    }

    /// Deletes an MCP server.
    ///
    /// `DELETE /api-catalog/mcp-servers/{id}`
    ///
    /// # Errors
    ///
    /// Returns an error if `id` is not valid or the request fails.
    pub async fn delete_mcp_server(&self, id: &str) -> Result<()> {
        validate_id(id)?;
        let url = self.url(id).await?;
        let request = self.inner.delete(&url).build().map_err(HttpError::from)?;
        self.inner
            .execute_and_check_success(request, "Delete MCP server failed")
            .await?;
        Ok(())
    }

    /// Connects to the MCP server and gets its current assets.
    ///
    /// `POST /api-catalog/mcp-servers/{id}/fetch` with body `{}`.
    ///
    /// # Errors
    ///
    /// Returns an error if `id` is not valid, the request fails, or the
    /// response cannot be decoded.
    pub async fn fetch_mcp_server(&self, id: &str) -> Result<McpServerFetchOutput> {
        validate_id(id)?;
        let url = self.url(&format!("{id}/fetch")).await?;
        // Send an explicit `{}`: the server rejects a POST with no body.
        let request = self.inner.post(&url).json(&serde_json::json!({}));
        self.send(request, "Fetch MCP server failed").await
    }

    /// Lists the saved assets of an MCP server.
    ///
    /// `GET /api-catalog/mcp-servers/{id}/assets`
    ///
    /// # Errors
    ///
    /// Returns an error if `id` is not valid, the request fails, or the
    /// response cannot be decoded.
    pub async fn list_mcp_server_assets(&self, id: &str) -> Result<McpServerAssetCollection> {
        validate_id(id)?;
        let url = self.url(&format!("{id}/assets")).await?;
        self.send(self.inner.get(&url), "List MCP server assets failed")
            .await
    }

    /// Replaces the saved assets of an MCP server.
    ///
    /// `PUT /api-catalog/mcp-servers/{id}/assets`
    ///
    /// # Errors
    ///
    /// Returns an error if `id` is not valid, the request fails, or the
    /// response cannot be decoded.
    pub async fn replace_mcp_server_assets(
        &self,
        id: &str,
        input: &McpServerAssetReplaceInput,
    ) -> Result<McpServerAssetCollection> {
        validate_id(id)?;
        let url = self.url(&format!("{id}/assets")).await?;
        let request = self.inner.put(&url).json(input);
        self.send(request, "Replace MCP server assets failed").await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::{ForceClient, builder};
    use crate::test_utils::mock_auth::MockAuthenticator;
    use crate::test_utils::must::Must;
    use serde_json::json;
    use wiremock::matchers::{body_json, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const BASE: &str = "/services/data/v68.0/api-catalog/mcp-servers";

    async fn setup() -> (MockServer, ForceClient<MockAuthenticator>) {
        let server = MockServer::start().await;
        let client = builder()
            .authenticate(MockAuthenticator::new("t", &server.uri()))
            .build()
            .await
            .must();
        (server, client)
    }

    fn server_json() -> serde_json::Value {
        json!({
            "id": "1mcxx0000000001AAA",
            "name": "weather",
            "label": "Weather",
            "type": "EXTERNAL",
            "status": "ACTIVE",
            "serverUrl": "https://mcp.example.com/mcp",
            "authorization": {"authType": "NO_AUTH"}
        })
    }

    fn asset_json() -> serde_json::Value {
        json!({"id": "1mdxx0000000001AAA", "name": "get_forecast", "kind": "MCP_TOOL", "active": true})
    }

    #[test]
    fn test_validate_id() {
        assert!(validate_id("1mcxx0000000001AAA").is_ok());
        assert!(validate_id("").is_err());
        assert!(validate_id("../x").is_err());
        assert!(validate_id("a/b").is_err());
    }

    #[tokio::test]
    async fn test_list_mcp_servers_with_filters() {
        let (server, client) = setup().await;
        Mock::given(method("GET"))
            .and(path(BASE))
            .and(query_param("status", "ACTIVE"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(json!({"mcpServers": [server_json()]})),
            )
            .expect(1)
            .mount(&server)
            .await;
        let opts = ListMcpServersOptions {
            status: Some(McpServerStatusFilter::Active),
            ..Default::default()
        };
        let out = client.api_catalog().list_mcp_servers(&opts).await.must();
        assert_eq!(out.mcp_servers[0].name, "weather");
        assert_eq!(out.mcp_servers[0].server_type, McpServerType::External);
    }

    #[tokio::test]
    async fn test_create_mcp_server() {
        let (server, client) = setup().await;
        Mock::given(method("POST"))
            .and(path(BASE))
            .and(body_json(json!({
                "name": "weather",
                "label": "Weather",
                "type": "EXTERNAL",
                "serverUrl": "https://mcp.example.com/mcp",
                "authorization": {"authType": "OAUTH", "clientId": "cid", "clientSecret": "sec"}
            })))
            .respond_with(ResponseTemplate::new(201).set_body_json(json!({
                "server": server_json(),
                "assets": [{"name": "get_forecast", "kind": "MCP_TOOL", "securityWarning": "w"}]
            })))
            .expect(1)
            .mount(&server)
            .await;
        let input = McpServerCreateInput::external("weather", "https://mcp.example.com/mcp")
            .with_label("Weather")
            .with_authorization(McpServerAuthorizationInput::oauth("cid", "sec"));

        let out = client.api_catalog().create_mcp_server(&input).await.must();
        assert_eq!(out.server.id, "1mcxx0000000001AAA");
        assert_eq!(out.assets[0].security_warning.as_deref(), Some("w"));
    }

    #[tokio::test]
    async fn test_get_update_delete() {
        let (server, client) = setup().await;
        let one = format!("{BASE}/1mcxx0000000001AAA");
        Mock::given(method("GET"))
            .and(path(one.as_str()))
            .respond_with(ResponseTemplate::new(200).set_body_json(server_json()))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("PUT"))
            .and(path(one.as_str()))
            .and(body_json(json!({"description": "d"})))
            .respond_with(ResponseTemplate::new(200).set_body_json(server_json()))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("DELETE"))
            .and(path(one.as_str()))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;
        let catalog = client.api_catalog();

        let got = catalog.get_mcp_server("1mcxx0000000001AAA").await.must();
        assert_eq!(got.status, "ACTIVE");
        let update = McpServerUpdateInput {
            description: Some("d".into()),
            ..Default::default()
        };
        catalog
            .update_mcp_server("1mcxx0000000001AAA", &update)
            .await
            .must();
        catalog.delete_mcp_server("1mcxx0000000001AAA").await.must();
    }

    #[tokio::test]
    async fn test_fetch_and_assets() {
        let (server, client) = setup().await;
        let one = format!("{BASE}/1mcxx0000000001AAA");
        Mock::given(method("POST"))
            .and(path(format!("{one}/fetch")))
            .and(body_json(json!({})))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({"assets": [{"name": "p", "kind": "MCP_PROMPT"}]})),
            )
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path(format!("{one}/assets")))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(json!({"assets": [asset_json()]})),
            )
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("PUT"))
            .and(path(format!("{one}/assets")))
            .and(body_json(
                json!({"assets": [{"name": "get_forecast", "active": false}]}),
            ))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(json!({"assets": [asset_json()]})),
            )
            .expect(1)
            .mount(&server)
            .await;
        let catalog = client.api_catalog();

        let fetched = catalog.fetch_mcp_server("1mcxx0000000001AAA").await.must();
        assert_eq!(fetched.assets[0].kind, McpAssetKind::McpPrompt);
        let saved = catalog
            .list_mcp_server_assets("1mcxx0000000001AAA")
            .await
            .must();
        assert!(saved.assets[0].active);
        let replace = McpServerAssetReplaceInput {
            assets: vec![McpServerAssetReplaceItem {
                name: Some("get_forecast".into()),
                active: Some(false),
                ..Default::default()
            }],
        };
        catalog
            .replace_mcp_server_assets("1mcxx0000000001AAA", &replace)
            .await
            .must();
    }

    #[tokio::test]
    async fn test_bad_id_sends_nothing() {
        let (server, client) = setup().await;
        let catalog = client.api_catalog();
        for bad in ["../etc", "", "a/b", "a?b"] {
            assert!(catalog.get_mcp_server(bad).await.is_err());
            assert!(catalog.delete_mcp_server(bad).await.is_err());
            assert!(
                catalog
                    .update_mcp_server(bad, &McpServerUpdateInput::default())
                    .await
                    .is_err()
            );
            assert!(catalog.fetch_mcp_server(bad).await.is_err());
            assert!(catalog.list_mcp_server_assets(bad).await.is_err());
            assert!(
                catalog
                    .replace_mcp_server_assets(bad, &McpServerAssetReplaceInput::default())
                    .await
                    .is_err()
            );
        }
        assert!(server.received_requests().await.must().is_empty());
    }

    #[tokio::test]
    async fn test_list_label_and_type_filters_and_delete_error() {
        let (server, client) = setup().await;
        Mock::given(method("GET"))
            .and(path(BASE))
            .and(query_param("label", "Weather"))
            .and(query_param("type", "EXTERNAL"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"mcpServers": []})))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("DELETE"))
            .and(path(format!("{BASE}/1mcxx0000000001AAA")))
            .respond_with(ResponseTemplate::new(404).set_body_json(json!([
                {"errorCode": "NOT_FOUND", "message": "gone"}
            ])))
            .mount(&server)
            .await;
        let opts = ListMcpServersOptions {
            label: Some("Weather".into()),
            server_type: Some(McpServerType::External),
            status: None,
        };
        let out = client.api_catalog().list_mcp_servers(&opts).await.must();
        assert_eq!(out.mcp_servers.len(), 0);
        assert!(
            client
                .api_catalog()
                .delete_mcp_server("1mcxx0000000001AAA")
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn test_server_error_maps_to_error() {
        let (server, client) = setup().await;
        Mock::given(method("GET"))
            .and(path(BASE))
            .respond_with(ResponseTemplate::new(400).set_body_json(json!([
                {"errorCode": "INVALID_INPUT", "message": "bad status"}
            ])))
            .mount(&server)
            .await;
        let err = client
            .api_catalog()
            .list_mcp_servers(&ListMcpServersOptions::default())
            .await
            .err()
            .must();
        assert!(err.to_string().contains("bad status"), "{err}");
    }
}
