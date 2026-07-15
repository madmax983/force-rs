//! Transactional Messaging API (single-recipient email and SMS sends).

use crate::client::MarketingCloudClient;
use crate::error::Result;
use crate::types::Attributes;
use reqwest::Method;
use serde::{Deserialize, Serialize};

/// A single message recipient with optional personalization attributes.
#[derive(Debug, Clone, Serialize)]
pub struct Recipient {
    /// The contact/subscriber key identifying the recipient.
    #[serde(rename = "contactKey")]
    pub contact_key: String,

    /// The destination address (email address, or E.164 phone number for SMS).
    pub to: String,

    /// Optional personalization name/value pairs merged into the template.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attributes: Option<Attributes>,
}

impl Recipient {
    /// Creates a recipient with no attributes.
    #[must_use]
    pub fn new(contact_key: impl Into<String>, to: impl Into<String>) -> Self {
        Self {
            contact_key: contact_key.into(),
            to: to.into(),
            attributes: None,
        }
    }

    /// Sets the personalization attributes.
    #[must_use]
    pub fn with_attributes(mut self, attributes: Attributes) -> Self {
        self.attributes = Some(attributes);
        self
    }
}

/// Request body for a single-recipient transactional email send.
#[derive(Debug, Clone, Serialize)]
pub struct SendEmailRequest {
    /// The send definition key to use.
    #[serde(rename = "definitionKey")]
    pub definition_key: String,

    /// The single recipient of this message.
    pub recipient: Recipient,
}

impl SendEmailRequest {
    /// Creates a new email send request.
    #[must_use]
    pub fn new(definition_key: impl Into<String>, recipient: Recipient) -> Self {
        Self {
            definition_key: definition_key.into(),
            recipient,
        }
    }
}

/// Request body for a single-recipient transactional SMS send.
#[derive(Debug, Clone, Serialize)]
pub struct SendSmsRequest {
    /// The SMS send definition key to use.
    #[serde(rename = "definitionKey")]
    pub definition_key: String,

    /// The single recipient of this message.
    pub recipient: Recipient,
}

impl SendSmsRequest {
    /// Creates a new SMS send request.
    #[must_use]
    pub fn new(definition_key: impl Into<String>, recipient: Recipient) -> Self {
        Self {
            definition_key: definition_key.into(),
            recipient,
        }
    }
}

/// One entry of a transactional send response.
#[derive(Debug, Clone, Deserialize)]
pub struct MessageResponseItem {
    /// The message key echoed back for this send.
    #[serde(rename = "messageKey", default)]
    pub message_key: Option<String>,

    /// The queue/acceptance status for this send (e.g. `"queued"`).
    #[serde(default)]
    pub status: Option<String>,
}

/// Response returned when accepting a transactional send.
#[derive(Debug, Clone, Deserialize)]
pub struct SendMessageResponse {
    /// The request id assigned to this send.
    #[serde(rename = "requestId", default)]
    pub request_id: Option<String>,

    /// Per-message acceptance details, when present.
    #[serde(default)]
    pub responses: Option<Vec<MessageResponseItem>>,
}

/// Handler for the Transactional Messaging API.
#[derive(Debug)]
pub struct TransactionalHandler<'a> {
    /// The owning client.
    client: &'a MarketingCloudClient,

    /// Optional per-call business unit override.
    account_id: Option<String>,
}

impl<'a> TransactionalHandler<'a> {
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

    /// Sends a transactional email to a single recipient.
    ///
    /// `POST messaging/v1/email/messages/{messageKey}`
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or is rejected by Marketing Cloud.
    pub async fn send_email(
        &self,
        message_key: &str,
        request: &SendEmailRequest,
    ) -> Result<SendMessageResponse> {
        let path = format!("messaging/v1/email/messages/{message_key}");
        let body = serde_json::to_value(request)?;
        self.client
            .request_typed(Method::POST, &path, self.account_id.as_deref(), Some(&body))
            .await
    }

    /// Retrieves the delivery status for a previously sent email message.
    ///
    /// `GET messaging/v1/email/messages/{messageKey}`
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or is rejected by Marketing Cloud.
    pub async fn email_status(&self, message_key: &str) -> Result<serde_json::Value> {
        let path = format!("messaging/v1/email/messages/{message_key}");
        self.client
            .request_raw(Method::GET, &path, self.account_id.as_deref(), None)
            .await
    }

    /// Sends a transactional SMS to a single recipient.
    ///
    /// `POST messaging/v1/sms/messages/{messageKey}`
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or is rejected by Marketing Cloud.
    pub async fn send_sms(
        &self,
        message_key: &str,
        request: &SendSmsRequest,
    ) -> Result<SendMessageResponse> {
        let path = format!("messaging/v1/sms/messages/{message_key}");
        let body = serde_json::to_value(request)?;
        self.client
            .request_typed(Method::POST, &path, self.account_id.as_deref(), Some(&body))
            .await
    }
}
