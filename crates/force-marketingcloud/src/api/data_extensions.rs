//! Data Extensions API (rowset upsert, async insert, and row queries).

use crate::client::MarketingCloudClient;
use crate::error::Result;
use reqwest::Method;
use serde::{Deserialize, Serialize};

/// A single data-extension row expressed as key and value field maps.
///
/// `keys` holds the primary-key field(s); `values` holds the remaining fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Row {
    /// Primary-key field name/value pairs.
    pub keys: serde_json::Map<String, serde_json::Value>,

    /// Non-key field name/value pairs.
    pub values: serde_json::Map<String, serde_json::Value>,
}

impl Row {
    /// Creates a row from key and value maps.
    #[must_use]
    pub fn new(
        keys: serde_json::Map<String, serde_json::Value>,
        values: serde_json::Map<String, serde_json::Value>,
    ) -> Self {
        Self { keys, values }
    }
}

/// Response returned when an asynchronous row operation is accepted.
#[derive(Debug, Clone, Deserialize)]
pub struct AsyncRowsResponse {
    /// The request id used to poll operation status.
    #[serde(rename = "requestId", default)]
    pub request_id: Option<String>,
}

/// Handler for the Data Extensions API.
#[derive(Debug)]
pub struct DataExtensionsHandler<'a> {
    /// The owning client.
    client: &'a MarketingCloudClient,

    /// Optional per-call business unit override.
    account_id: Option<String>,
}

impl<'a> DataExtensionsHandler<'a> {
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

    /// Synchronously upserts rows into a data extension by external key.
    ///
    /// `POST hub/v1/dataevents/key:{key}/rowset`
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or is rejected by Marketing Cloud.
    pub async fn upsert_rows(&self, external_key: &str, rows: &[Row]) -> Result<serde_json::Value> {
        let path = format!("hub/v1/dataevents/key:{external_key}/rowset");
        let body = serde_json::to_value(rows)?;
        self.client
            .request_raw(Method::POST, &path, self.account_id.as_deref(), Some(&body))
            .await
    }

    /// Asynchronously inserts rows into a data extension by external key.
    ///
    /// `POST data/v1/async/dataextensions/key:{key}/rows`
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or is rejected by Marketing Cloud.
    pub async fn insert_rows_async(
        &self,
        external_key: &str,
        rows: &[Row],
    ) -> Result<AsyncRowsResponse> {
        let path = format!("data/v1/async/dataextensions/key:{external_key}/rows");
        let items: Vec<serde_json::Value> = rows
            .iter()
            .map(|row| {
                // ⚡ Bolt: Collecting keys and values concurrently using iter().chain() avoids the intermediate `.clone()` allocations for both maps.
                let merged: serde_json::Map<String, serde_json::Value> = row
                    .keys
                    .iter()
                    .chain(row.values.iter())
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                serde_json::Value::Object(merged)
            })
            .collect();
        let body = serde_json::json!({ "items": items });
        self.client
            .request_typed(Method::POST, &path, self.account_id.as_deref(), Some(&body))
            .await
    }

    /// Queries rows from a data extension by external key.
    ///
    /// `GET data/v1/customobjectdata/key/{key}/rowset`
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or is rejected by Marketing Cloud.
    pub async fn query_rows(&self, external_key: &str) -> Result<serde_json::Value> {
        let path = format!("data/v1/customobjectdata/key/{external_key}/rowset");
        self.client
            .request_raw(Method::GET, &path, self.account_id.as_deref(), None)
            .await
    }
}
