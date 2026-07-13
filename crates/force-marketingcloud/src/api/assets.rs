//! Content Builder Assets API.

use crate::client::MarketingCloudClient;
use crate::error::Result;
use crate::types::Paged;
use reqwest::Method;
use serde::{Deserialize, Serialize};

/// Identifies a Content Builder asset type (e.g. `htmlemail` = 208).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetType {
    /// The numeric asset type id.
    pub id: i64,

    /// The asset type slug/name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl AssetType {
    /// Creates an asset type from an id and slug.
    #[must_use]
    pub fn new(id: i64, name: impl Into<String>) -> Self {
        Self {
            id,
            name: Some(name.into()),
        }
    }
}

/// A Content Builder asset (create request / read response).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Asset {
    /// The numeric asset id (present on responses).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,

    /// The display name of the asset.
    pub name: String,

    /// The asset type descriptor.
    #[serde(rename = "assetType")]
    pub asset_type: AssetType,

    /// An external unique key for the asset.
    #[serde(
        rename = "customerKey",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub customer_key: Option<String>,

    /// Raw content for simple asset types.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,

    /// Structured sub-views for email assets (html/text/subjectline/preheader).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub views: Option<serde_json::Value>,

    /// Channel availability flags (e.g. `{ "email": true }`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channels: Option<serde_json::Value>,

    /// Target Content Builder folder category.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<serde_json::Value>,
}

impl Asset {
    /// Creates a minimal asset with a name and type.
    #[must_use]
    pub fn new(name: impl Into<String>, asset_type: AssetType) -> Self {
        Self {
            id: None,
            name: name.into(),
            asset_type,
            customer_key: None,
            content: None,
            views: None,
            channels: None,
            category: None,
        }
    }
}

/// Handler for the Content Builder Assets API.
#[derive(Debug)]
pub struct AssetsHandler<'a> {
    /// The owning client.
    client: &'a MarketingCloudClient,

    /// Optional per-call business unit override.
    account_id: Option<String>,
}

impl<'a> AssetsHandler<'a> {
    /// Creates a new handler bound to `client`.
    pub(crate) const fn new(client: &'a MarketingCloudClient) -> Self {
        Self {
            client,
            account_id: None,
        }
    }

    /// Scopes subsequent calls to a specific business unit (MID).
    #[must_use]
    pub fn for_business_unit(mut self, account_id: impl Into<String>) -> Self {
        self.account_id = Some(account_id.into());
        self
    }

    /// Creates a new asset.
    ///
    /// `POST asset/v1/content/assets`
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or is rejected by Marketing Cloud.
    pub async fn create(&self, asset: &Asset) -> Result<Asset> {
        let body = serde_json::to_value(asset)?;
        self.client
            .request_typed(
                Method::POST,
                "asset/v1/content/assets",
                self.account_id.as_deref(),
                Some(&body),
            )
            .await
    }

    /// Retrieves a single asset by id.
    ///
    /// `GET asset/v1/content/assets/{id}`
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or is rejected by Marketing Cloud.
    pub async fn get(&self, id: i64) -> Result<Asset> {
        let path = format!("asset/v1/content/assets/{id}");
        self.client
            .request_typed(Method::GET, &path, self.account_id.as_deref(), None)
            .await
    }

    /// Applies a partial update to an asset.
    ///
    /// `PATCH asset/v1/content/assets/{id}`
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or is rejected by Marketing Cloud.
    pub async fn update(&self, id: i64, patch: &serde_json::Value) -> Result<Asset> {
        let path = format!("asset/v1/content/assets/{id}");
        self.client
            .request_typed(
                Method::PATCH,
                &path,
                self.account_id.as_deref(),
                Some(patch),
            )
            .await
    }

    /// Deletes an asset by id.
    ///
    /// `DELETE asset/v1/content/assets/{id}`
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or is rejected by Marketing Cloud.
    pub async fn delete(&self, id: i64) -> Result<()> {
        let path = format!("asset/v1/content/assets/{id}");
        self.client
            .request_raw(Method::DELETE, &path, self.account_id.as_deref(), None)
            .await?;
        Ok(())
    }

    /// Lists assets as a simple collection.
    ///
    /// `GET asset/v1/content/assets`
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or is rejected by Marketing Cloud.
    pub async fn list(&self) -> Result<Paged<Asset>> {
        self.client
            .request_typed(
                Method::GET,
                "asset/v1/content/assets",
                self.account_id.as_deref(),
                None,
            )
            .await
    }
}
