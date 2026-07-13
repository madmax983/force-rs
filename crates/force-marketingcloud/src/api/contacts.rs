//! Contacts API.

use crate::client::MarketingCloudClient;
use crate::error::Result;
use reqwest::Method;
use serde::{Deserialize, Serialize};

/// A Marketing Cloud contact, keyed by the caller-owned `contactKey`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contact {
    /// The caller-owned unique contact key.
    #[serde(rename = "contactKey")]
    pub contact_key: String,

    /// Attribute group objects carrying the contact's field values.
    #[serde(rename = "attributeSets", default = "Vec::new")]
    pub attribute_sets: Vec<serde_json::Value>,
}

impl Contact {
    /// Creates a contact with the given key and no attribute sets.
    #[must_use]
    pub fn new(contact_key: impl Into<String>) -> Self {
        Self {
            contact_key: contact_key.into(),
            attribute_sets: Vec::new(),
        }
    }

    /// Sets the attribute sets for this contact.
    #[must_use]
    pub fn with_attribute_sets(mut self, attribute_sets: Vec<serde_json::Value>) -> Self {
        self.attribute_sets = attribute_sets;
        self
    }
}

/// Handler for the Contacts API.
#[derive(Debug)]
pub struct ContactsHandler<'a> {
    /// The owning client.
    client: &'a MarketingCloudClient,

    /// Optional per-call business unit override.
    account_id: Option<String>,
}

impl<'a> ContactsHandler<'a> {
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

    /// Creates a contact.
    ///
    /// `POST contacts/v1/contacts`
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or is rejected by Marketing Cloud.
    pub async fn create(&self, contact: &Contact) -> Result<serde_json::Value> {
        let body = serde_json::to_value(contact)?;
        self.client
            .request_raw(
                Method::POST,
                "contacts/v1/contacts",
                self.account_id.as_deref(),
                Some(&body),
            )
            .await
    }

    /// Deletes contacts by their contact keys (asynchronous on the server).
    ///
    /// `POST contacts/v1/contacts/actions/delete?type=keys`
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or is rejected by Marketing Cloud.
    pub async fn delete_by_key(&self, contact_key: &str) -> Result<serde_json::Value> {
        let body = serde_json::json!({
            "values": [contact_key],
            "DeleteOperationType": "ContactAndAttributes"
        });
        self.client
            .request_raw(
                Method::POST,
                "contacts/v1/contacts/actions/delete?type=keys",
                self.account_id.as_deref(),
                Some(&body),
            )
            .await
    }
}
