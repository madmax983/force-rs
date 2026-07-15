//! Request and response types for the Agentforce Agent API.
//!
//! Response types keep Trust Layer, action-result, and citation fields
//! permissive (`Option<T>` / [`serde_json::Value`]). The agent `messages[]`
//! array is polymorphic — its `type` field drives the shape — so `type` is
//! modeled as an open `String`, never a closed enum.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The org endpoint (My Domain URL) the agent runs against.
#[derive(Debug, Clone, Serialize)]
pub struct InstanceConfig {
    /// The org's My Domain URL, e.g. `https://mydomain.my.salesforce.com`.
    pub endpoint: String,
}

/// Streaming capabilities advertised at session start.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamingCapabilities {
    /// The chunk types the client can handle, e.g. `["Text"]`.
    pub chunk_types: Vec<String>,
}

/// A context variable passed to the agent.
#[derive(Debug, Clone, Serialize)]
pub struct AgentVariable {
    /// Variable name.
    pub name: String,
    /// Variable type (e.g. `Text`).
    #[serde(rename = "type")]
    pub var_type: String,
    /// Variable value (free-form).
    pub value: Value,
}

impl AgentVariable {
    /// Creates a new agent variable.
    #[must_use]
    pub fn new(name: impl Into<String>, var_type: impl Into<String>, value: Value) -> Self {
        Self {
            name: name.into(),
            var_type: var_type.into(),
            value,
        }
    }
}

/// Request body for starting an agent session.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartSessionRequest {
    /// Client-generated UUID used to trace the conversation.
    pub external_session_key: String,
    /// The org endpoint configuration.
    pub instance_config: InstanceConfig,
    /// Optional streaming capabilities (e.g. `["Text"]`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub streaming_capabilities: Option<StreamingCapabilities>,
    /// When `true`, run as the agent-assigned user instead of the logged-in user.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bypass_user: Option<bool>,
    /// Optional context variables passed at session start.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variables: Option<Vec<AgentVariable>>,
}

impl StartSessionRequest {
    /// Creates a new start-session request with `Text` streaming enabled.
    #[must_use]
    pub fn new(external_session_key: impl Into<String>, endpoint: impl Into<String>) -> Self {
        Self {
            external_session_key: external_session_key.into(),
            instance_config: InstanceConfig {
                endpoint: endpoint.into(),
            },
            streaming_capabilities: Some(StreamingCapabilities {
                chunk_types: vec!["Text".to_owned()],
            }),
            bypass_user: None,
            variables: None,
        }
    }

    /// Sets whether to bypass the logged-in user.
    #[must_use]
    pub fn with_bypass_user(mut self, bypass_user: bool) -> Self {
        self.bypass_user = Some(bypass_user);
        self
    }

    /// Sets the context variables.
    #[must_use]
    pub fn with_variables(mut self, variables: Vec<AgentVariable>) -> Self {
        self.variables = Some(variables);
        self
    }
}

/// A single message in an agent response.
///
/// The `messages[]` array is polymorphic; `type` (e.g. `Inform`,
/// `ProgressIndicator`, `Error`) drives which fields are populated. Only
/// `message_type` is guaranteed present.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentMessage {
    /// The message type, e.g. `Inform`.
    #[serde(rename = "type")]
    pub message_type: String,
    /// Message id.
    #[serde(default)]
    pub id: Option<String>,
    /// The agent's text reply (present on `Inform`).
    #[serde(default)]
    pub message: Option<String>,
    /// Id used to submit feedback on this message (may be empty).
    #[serde(default)]
    pub feedback_id: Option<String>,
    /// Planner run id (may be empty).
    #[serde(default)]
    pub plan_id: Option<String>,
    /// Trust Layer content-safety flag.
    #[serde(default)]
    pub is_content_safe: Option<bool>,
    /// Structured action results (free-form).
    #[serde(default)]
    pub result: Vec<Value>,
    /// Grounding/citation references (free-form).
    #[serde(default)]
    pub cited_references: Vec<Value>,
}

/// Response body for starting an agent session.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartSessionResponse {
    /// The new session id, required for all later calls.
    pub session_id: String,
    /// The agent's greeting message(s).
    #[serde(default)]
    pub messages: Vec<AgentMessage>,
    /// HATEOAS links (`self`, `messages`, `messagesStream`, `session`, `end`).
    #[serde(rename = "_links", default)]
    pub links: Option<Value>,
}

/// An outbound message sent to the agent.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutboundMessage {
    /// Monotonically increasing per-session sequence id.
    pub sequence_id: i64,
    /// Message type, e.g. `Text`.
    #[serde(rename = "type")]
    pub message_type: String,
    /// The user's utterance.
    pub text: String,
}

/// Request body for sending a synchronous message.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SendMessageRequest {
    /// The message to send.
    pub message: OutboundMessage,
    /// Optional per-turn context variables.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variables: Option<Vec<AgentVariable>>,
}

impl SendMessageRequest {
    /// Creates a text message request with the given sequence id and text.
    #[must_use]
    pub fn text(sequence_id: i64, text: impl Into<String>) -> Self {
        Self {
            message: OutboundMessage {
                sequence_id,
                message_type: "Text".to_owned(),
                text: text.into(),
            },
            variables: None,
        }
    }

    /// Sets the per-turn context variables.
    #[must_use]
    pub fn with_variables(mut self, variables: Vec<AgentVariable>) -> Self {
        self.variables = Some(variables);
        self
    }
}

/// Response body for sending a synchronous message.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentResponse {
    /// The agent's reply message(s).
    #[serde(default)]
    pub messages: Vec<AgentMessage>,
    /// HATEOAS links.
    #[serde(rename = "_links", default)]
    pub links: Option<Value>,
}

impl AgentResponse {
    /// Returns the agent's first reply text, if present.
    #[must_use]
    pub fn reply_text(&self) -> Option<&str> {
        self.messages.first().and_then(|m| m.message.as_deref())
    }
}

/// Reason supplied when ending an agent session.
///
/// Serialized to the raw string sent in the `x-session-end-reason` header.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum SessionEndReason {
    /// The user requested the session end (default).
    #[default]
    UserRequest,
    /// The session was transferred (e.g. to a human).
    Transfer,
    /// The session ended due to an error.
    Error,
    /// Any other reason, serialized verbatim.
    Other(String),
}

impl SessionEndReason {
    /// Returns the raw header value for this reason.
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::UserRequest => "UserRequest",
            Self::Transfer => "Transfer",
            Self::Error => "Error",
            Self::Other(s) => s.as_str(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::must::Must;
    use serde_json::json;

    #[test]
    fn test_start_session_request_serialization() {
        let req = StartSessionRequest::new("uuid-123", "https://my.salesforce.com")
            .with_bypass_user(true);
        let v = serde_json::to_value(&req).must();
        assert_eq!(v["externalSessionKey"], "uuid-123");
        assert_eq!(v["instanceConfig"]["endpoint"], "https://my.salesforce.com");
        assert_eq!(v["streamingCapabilities"]["chunkTypes"][0], "Text");
        assert_eq!(v["bypassUser"], true);
        assert!(v.get("variables").is_none());
    }

    #[test]
    fn test_send_message_request_text() {
        let req = SendMessageRequest::text(1, "How do I reset my password?");
        let v = serde_json::to_value(&req).must();
        assert_eq!(v["message"]["sequenceId"], 1);
        assert_eq!(v["message"]["type"], "Text");
        assert_eq!(v["message"]["text"], "How do I reset my password?");
    }

    #[test]
    fn test_agent_variable_serialization() {
        let var = AgentVariable::new("CustomerName", "Text", json!("Acme"));
        let v = serde_json::to_value(&var).must();
        assert_eq!(v["name"], "CustomerName");
        assert_eq!(v["type"], "Text");
        assert_eq!(v["value"], "Acme");
    }

    #[test]
    fn test_start_session_response_deserialization() {
        let json_str = r#"{
            "sessionId": "80ab2e16-392e-4c12-b80a-f028a58400b5",
            "_links": {"messages": {"href": "https://x/messages"}},
            "messages": [
                {"type": "Inform", "id": "d27b", "feedbackId": "", "planId": "",
                 "isContentSafe": true, "message": "Hi, how can I help?", "result": [],
                 "citedReferences": []}
            ]
        }"#;
        let resp: StartSessionResponse = serde_json::from_str(json_str).must();
        assert_eq!(resp.session_id, "80ab2e16-392e-4c12-b80a-f028a58400b5");
        assert_eq!(resp.messages[0].message_type, "Inform");
        assert_eq!(
            resp.messages[0].message.as_deref(),
            Some("Hi, how can I help?")
        );
        assert!(resp.links.is_some());
    }

    #[test]
    fn test_agent_response_reply_text() {
        let json_str = r#"{
            "messages": [
                {"type": "Inform", "message": "To reset your password, go to ..."}
            ],
            "_links": {}
        }"#;
        let resp: AgentResponse = serde_json::from_str(json_str).must();
        assert_eq!(resp.reply_text(), Some("To reset your password, go to ..."));
    }

    #[test]
    fn test_session_end_reason_as_str_and_default() {
        assert_eq!(SessionEndReason::default(), SessionEndReason::UserRequest);
        assert_eq!(SessionEndReason::UserRequest.as_str(), "UserRequest");
        assert_eq!(SessionEndReason::Transfer.as_str(), "Transfer");
        assert_eq!(SessionEndReason::Error.as_str(), "Error");
        assert_eq!(
            SessionEndReason::Other("Expiration".to_owned()).as_str(),
            "Expiration"
        );
    }
}
