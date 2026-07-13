//! Journeys (Interaction) API.

use crate::client::MarketingCloudClient;
use crate::error::Result;
use crate::types::Paged;
use reqwest::Method;
use serde::{Deserialize, Serialize};

/// A journey (interaction) summary as returned by the list endpoint.
#[derive(Debug, Clone, Deserialize)]
pub struct Journey {
    /// The journey id (GUID).
    #[serde(default)]
    pub id: Option<String>,

    /// The journey key.
    #[serde(default)]
    pub key: Option<String>,

    /// The journey display name.
    #[serde(default)]
    pub name: Option<String>,

    /// The journey version number.
    #[serde(default)]
    pub version: Option<i64>,

    /// The publication status (e.g. `"Published"`).
    #[serde(default)]
    pub status: Option<String>,
}

/// Request body for firing a journey entry event.
#[derive(Debug, Clone, Serialize)]
pub struct InteractionEvent {
    /// The subscriber/contact unique id to enter into the journey.
    #[serde(rename = "ContactKey")]
    pub contact_key: String,

    /// The API entry event definition key (must not contain `.`).
    #[serde(rename = "EventDefinitionKey")]
    pub event_definition_key: String,

    /// Optional merge/entry data for the event.
    #[serde(rename = "Data", skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl InteractionEvent {
    /// Creates an interaction event with no data payload.
    #[must_use]
    pub fn new(contact_key: impl Into<String>, event_definition_key: impl Into<String>) -> Self {
        Self {
            contact_key: contact_key.into(),
            event_definition_key: event_definition_key.into(),
            data: None,
        }
    }

    /// Attaches a data payload to the event.
    #[must_use]
    pub fn with_data(mut self, data: serde_json::Value) -> Self {
        self.data = Some(data);
        self
    }
}

/// Response returned when a journey entry event is created.
#[derive(Debug, Clone, Deserialize)]
pub struct FireEventResponse {
    /// The id of the created event instance.
    #[serde(rename = "eventInstanceId", default)]
    pub event_instance_id: Option<String>,
}

/// Handler for the Journeys (Interaction) API.
#[derive(Debug)]
pub struct JourneysHandler<'a> {
    /// The owning client.
    client: &'a MarketingCloudClient,

    /// Optional per-call business unit override.
    account_id: Option<String>,
}

impl<'a> JourneysHandler<'a> {
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

    /// Lists journeys (interactions).
    ///
    /// `GET interaction/v1/interactions`
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or is rejected by Marketing Cloud.
    pub async fn list(&self) -> Result<Paged<Journey>> {
        self.client
            .request_typed(
                Method::GET,
                "interaction/v1/interactions",
                self.account_id.as_deref(),
                None,
            )
            .await
    }

    /// Fires a journey entry event, adding a contact to an API-event journey.
    ///
    /// `POST interaction/v1/events`
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or is rejected by Marketing Cloud.
    pub async fn fire_event(&self, event: &InteractionEvent) -> Result<FireEventResponse> {
        let body = serde_json::to_value(event)?;
        self.client
            .request_typed(
                Method::POST,
                "interaction/v1/events",
                self.account_id.as_deref(),
                Some(&body),
            )
            .await
    }
}
