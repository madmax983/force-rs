//! API Catalog request and response types.
//!
//! Enums use an `Unknown` fallback, so new server values do not break decode.

use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize, Serializer};

/// MCP server type. Only `EXTERNAL` exists today.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum McpServerType {
    /// A server outside Salesforce.
    External,
    /// A value that this crate does not know.
    #[serde(other)]
    Unknown,
}

/// MCP server authorization type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum McpAuthType {
    /// OAuth 2.0.
    Oauth,
    /// No authorization.
    NoAuth,
    /// A value that this crate does not know.
    #[serde(other)]
    Unknown,
}

/// Kind of MCP asset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum McpAssetKind {
    /// MCP tool.
    McpTool,
    /// MCP prompt.
    McpPrompt,
    /// MCP resource.
    McpResource,
    /// A value that this crate does not know.
    #[serde(other)]
    Unknown,
}

/// Status filter for [`list_mcp_servers`](super::ApiCatalogHandler::list_mcp_servers).
///
/// The server accepts only these two values as a filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum McpServerStatusFilter {
    /// Connected servers.
    Active,
    /// Disconnected servers.
    Disconnected,
}

impl McpServerStatusFilter {
    /// Returns the wire value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "ACTIVE",
            Self::Disconnected => "DISCONNECTED",
        }
    }
}

/// Filters for listing MCP servers.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ListMcpServersOptions {
    /// Label filter.
    pub label: Option<String>,
    /// Type filter. Only `External` is sent; `Unknown` sends no filter.
    pub server_type: Option<McpServerType>,
    /// Status filter.
    pub status: Option<McpServerStatusFilter>,
}

impl ListMcpServersOptions {
    pub(crate) fn query_params(&self) -> Vec<(&'static str, String)> {
        let mut params = Vec::new();
        if let Some(label) = &self.label {
            params.push(("label", label.clone()));
        }
        if self.server_type == Some(McpServerType::External) {
            params.push(("type", "EXTERNAL".to_owned()));
        }
        if let Some(status) = self.status {
            params.push(("status", status.as_str().to_owned()));
        }
        params
    }
}

const fn unknown_server_type() -> McpServerType {
    McpServerType::Unknown
}

/// Authorization settings of an MCP server (output).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerAuthorization {
    /// Authorization type.
    pub auth_type: McpAuthType,
    /// Identity provider name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity_provider: Option<String>,
    /// OAuth scope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
}

#[allow(clippy::ref_option, reason = "serde serialize_with passes &Option<T>")]
fn serialize_secret<S: Serializer>(
    secret: &Option<SecretString>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    match secret {
        Some(s) => serializer.serialize_str(s.expose_secret()),
        None => serializer.serialize_none(),
    }
}

/// Authorization settings of an MCP server (input).
///
/// `client_secret` is a secret. `Debug` output hides it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerAuthorizationInput {
    /// Authorization type.
    pub auth_type: McpAuthType,
    /// Identity provider name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identity_provider: Option<String>,
    /// OAuth client ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
    /// OAuth client secret.
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_secret"
    )]
    pub client_secret: Option<SecretString>,
    /// OAuth scope.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
}

impl McpServerAuthorizationInput {
    /// Creates input with no authorization.
    #[must_use]
    pub const fn no_auth() -> Self {
        Self {
            auth_type: McpAuthType::NoAuth,
            identity_provider: None,
            client_id: None,
            client_secret: None,
            scope: None,
        }
    }

    /// Creates OAuth input with a client ID and secret.
    #[must_use]
    pub fn oauth(client_id: impl Into<String>, client_secret: impl Into<String>) -> Self {
        Self {
            auth_type: McpAuthType::Oauth,
            identity_provider: None,
            client_id: Some(client_id.into()),
            client_secret: Some(SecretString::new(client_secret.into().into())),
            scope: None,
        }
    }
}

/// MCP server record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServer {
    /// Record ID.
    pub id: String,
    /// Unique name.
    pub name: String,
    /// Label.
    #[serde(default)]
    pub label: Option<String>,
    /// Description.
    #[serde(default)]
    pub description: Option<String>,
    /// Server type. `Unknown` if the server omits it.
    #[serde(default = "unknown_server_type", rename = "type")]
    pub server_type: McpServerType,
    /// Connection status, for example `ACTIVE`, `INCOMPLETE`, or `DISCONNECTED`.
    #[serde(default)]
    pub status: String,
    /// Server URL.
    #[serde(default)]
    pub server_url: Option<String>,
    /// Authorization settings.
    #[serde(default)]
    pub authorization: Option<McpServerAuthorization>,
    /// Creator user ID.
    #[serde(default)]
    pub created_by_id: Option<String>,
    /// Creation time.
    #[serde(default)]
    pub created_date: Option<String>,
    /// Last modifier user ID.
    #[serde(default)]
    pub last_modified_by_id: Option<String>,
    /// Last change time.
    #[serde(default)]
    pub last_modified_date: Option<String>,
}

/// List of MCP servers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerCollection {
    /// Servers.
    #[serde(default)]
    pub mcp_servers: Vec<McpServer>,
}

/// Input to register an MCP server.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerCreateInput {
    /// Unique name.
    pub name: String,
    /// Label.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Server type.
    #[serde(rename = "type")]
    pub server_type: McpServerType,
    /// Server URL.
    pub server_url: String,
    /// Authorization settings.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorization: Option<McpServerAuthorizationInput>,
}

impl McpServerCreateInput {
    /// Creates input for an external server at `server_url`.
    #[must_use]
    pub fn external(name: impl Into<String>, server_url: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            label: None,
            description: None,
            server_type: McpServerType::External,
            server_url: server_url.into(),
            authorization: None,
        }
    }

    /// Sets the label.
    #[must_use]
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Sets the description.
    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Sets the authorization settings.
    #[must_use]
    pub fn with_authorization(mut self, authorization: McpServerAuthorizationInput) -> Self {
        self.authorization = Some(authorization);
        self
    }
}

/// Input to change an MCP server. Unset fields stay the same.
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct McpServerUpdateInput {
    /// Label.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Server URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_url: Option<String>,
    /// Authorization settings.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorization: Option<McpServerAuthorizationInput>,
}

/// Asset that the server reported during a fetch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpFetchedAsset {
    /// Record ID, if saved.
    #[serde(default)]
    pub id: Option<String>,
    /// Asset name.
    pub name: String,
    /// Label.
    #[serde(default)]
    pub label: Option<String>,
    /// Description.
    #[serde(default)]
    pub description: Option<String>,
    /// Asset kind.
    pub kind: McpAssetKind,
    /// Active flag.
    #[serde(default)]
    pub active: Option<bool>,
    /// True if agents can use the asset as an action.
    #[serde(default)]
    pub available_as_agent_action: Option<bool>,
    /// Status.
    #[serde(default)]
    pub status: Option<String>,
    /// Security warning from the server.
    #[serde(default)]
    pub security_warning: Option<String>,
}

/// Result of registering an MCP server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpServerCreateOutput {
    /// The new server.
    pub server: McpServer,
    /// Assets found on the server.
    #[serde(default)]
    pub assets: Vec<McpFetchedAsset>,
}

/// Result of fetching assets from an MCP server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpServerFetchOutput {
    /// Assets found on the server.
    #[serde(default)]
    pub assets: Vec<McpFetchedAsset>,
}

/// Saved asset of an MCP server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerAsset {
    /// Record ID.
    pub id: String,
    /// Asset name.
    pub name: String,
    /// Label.
    #[serde(default)]
    pub label: Option<String>,
    /// Description.
    #[serde(default)]
    pub description: Option<String>,
    /// Asset kind.
    pub kind: McpAssetKind,
    /// Active flag.
    #[serde(default)]
    pub active: bool,
    /// True if agents can use the asset as an action.
    #[serde(default)]
    pub available_as_agent_action: Option<bool>,
}

/// List of saved assets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpServerAssetCollection {
    /// Assets.
    #[serde(default)]
    pub assets: Vec<McpServerAsset>,
}

/// One item of an asset replace request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct McpServerAssetReplaceItem {
    /// Record ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Asset name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Label.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Active flag.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active: Option<bool>,
    /// Asset kind.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<McpAssetKind>,
}

/// Request to replace the saved assets of an MCP server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct McpServerAssetReplaceInput {
    /// The full asset list.
    pub assets: Vec<McpServerAssetReplaceItem>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::must::Must;
    use serde_json::json;

    #[test]
    fn test_unknown_enum_values_decode() {
        let s: McpServer = serde_json::from_value(json!({
            "id": "1", "name": "n", "type": "INTERNAL_NEW", "status": "CUSTOM",
            "authorization": {"authType": "MTLS"}
        }))
        .must();
        assert_eq!(s.server_type, McpServerType::Unknown);
        assert_eq!(s.authorization.must().auth_type, McpAuthType::Unknown);
        let a: McpFetchedAsset =
            serde_json::from_value(json!({"name": "x", "kind": "MCP_APP"})).must();
        assert_eq!(a.kind, McpAssetKind::Unknown);
    }

    #[test]
    fn test_missing_type_decodes_unknown() {
        let s: McpServer = serde_json::from_value(json!({"id": "1", "name": "n"})).must();
        assert_eq!(s.server_type, McpServerType::Unknown);
    }

    #[test]
    fn test_create_input_builders() {
        let input = McpServerCreateInput::external("w", "https://m")
            .with_label("W")
            .with_description("d")
            .with_authorization(McpServerAuthorizationInput::no_auth());
        let json = serde_json::to_value(&input).must();
        assert_eq!(json["label"], "W");
        assert_eq!(json["description"], "d");
        assert_eq!(json["authorization"]["authType"], "NO_AUTH");
        assert_eq!(json["type"], "EXTERNAL");
    }

    #[test]
    fn test_unknown_type_filter_is_not_sent() {
        let opts = ListMcpServersOptions {
            server_type: Some(McpServerType::Unknown),
            ..Default::default()
        };
        assert_eq!(opts.query_params().len(), 0);
    }

    #[test]
    fn test_oauth_input_serializes_secret_but_hides_it_in_debug() {
        let auth = McpServerAuthorizationInput::oauth("cid", "s3cr3t");
        let json = serde_json::to_value(&auth).must();
        assert_eq!(json["authType"], "OAUTH");
        assert_eq!(json["clientSecret"], "s3cr3t");
        assert!(!format!("{auth:?}").contains("s3cr3t"));
    }

    #[test]
    fn test_no_auth_input_omits_unset_fields() {
        let json = serde_json::to_value(McpServerAuthorizationInput::no_auth()).must();
        assert_eq!(json, json!({"authType": "NO_AUTH"}));
    }

    #[test]
    fn test_list_options_query_params() {
        let opts = ListMcpServersOptions {
            label: Some("Weather".into()),
            server_type: Some(McpServerType::External),
            status: Some(McpServerStatusFilter::Disconnected),
        };
        assert_eq!(
            opts.query_params(),
            vec![
                ("label", "Weather".to_owned()),
                ("type", "EXTERNAL".to_owned()),
                ("status", "DISCONNECTED".to_owned())
            ]
        );
        assert_eq!(ListMcpServersOptions::default().query_params().len(), 0);
    }

    #[test]
    fn test_update_input_sends_only_set_fields() {
        let input = McpServerUpdateInput {
            label: Some("New".into()),
            ..Default::default()
        };
        assert_eq!(serde_json::to_value(&input).must(), json!({"label": "New"}));
    }
}
