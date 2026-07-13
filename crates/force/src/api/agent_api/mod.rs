//! Agentforce Agent API handler.
//!
//! The Agent API drives headless conversations with an Agentforce agent:
//! start a session, exchange messages synchronously, and end the session.
//! Like the Models API, it targets the fixed host [`AI_PLATFORM_HOST`]
//! (`https://api.salesforce.com`) with a version-less path
//! (`/einstein/ai-agent/v1/`), not the org `instance_url`. The org's My Domain
//! URL is instead passed in the request body as `instanceConfig.endpoint`.
//!
//! # Feature Flag
//!
//! This module requires the `agent_api` feature flag:
//! ```toml
//! [dependencies]
//! force = { version = "...", features = ["agent_api"] }
//! ```
//!
//! # Authentication
//!
//! The Agent API reuses the org's standard OAuth 2.0 bearer token (for example
//! the client-credentials flow); no token exchange is required. The agent must
//! be linked to an **External Client App** (not a classic connected app), and
//! that app is the OAuth client whose token is used — the wire auth flow is
//! otherwise identical to the other handlers.
//!
//! # Streaming
//!
//! Only the synchronous message endpoint is implemented. The streaming SSE
//! endpoint (`.../messages/stream`) is a documented follow-up and is
//! intentionally out of scope here.
//!
//! # Government Cloud
//!
//! Override the host with [`AgentHandler::with_host`] (for example
//! `https://api.gov.salesforce.com`).
//!
//! # Usage
//!
//! ```ignore
//! let client = builder().authenticate(auth).build().await?;
//! let agents = client.agents();
//!
//! let session = agents.start_session_default("0XxHr000000ysOSKAY").await?;
//! let reply = agents.send_text(&session.session_id, 1, "How do I reset my password?").await?;
//! println!("{:?}", reply.reply_text());
//! agents.end_session(&session.session_id, SessionEndReason::UserRequest).await?;
//! ```

pub(crate) mod types;

pub use types::{
    AgentMessage, AgentResponse, AgentVariable, InstanceConfig, OutboundMessage,
    SendMessageRequest, SessionEndReason, StartSessionRequest, StartSessionResponse,
    StreamingCapabilities,
};

use crate::error::{HttpError, Result};
use std::sync::Arc;

/// Fixed host for the Einstein AI Platform APIs.
///
/// Override per-handler via [`AgentHandler::with_host`] for Government Cloud
/// (`https://api.gov.salesforce.com`) or testing.
pub const AI_PLATFORM_HOST: &str = "https://api.salesforce.com";

/// Header carrying the session-end reason on the `end_session` call.
const SESSION_END_REASON_HEADER: &str = "x-session-end-reason";

/// Agentforce Agent API handler.
///
/// Provides typed access to the headless Agent API (start session, send
/// message, end session) on [`AI_PLATFORM_HOST`].
///
/// The agent must be linked to an **External Client App** whose OAuth token
/// backs this handler.
///
/// Obtain a handler from [`ForceClient::agents`](crate::client::ForceClient::agents).
#[derive(Debug)]
pub struct AgentHandler<A: crate::auth::Authenticator> {
    inner: Arc<crate::session::Session<A>>,
    host: String,
}

impl<A: crate::auth::Authenticator> Clone for AgentHandler<A> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            host: self.host.clone(),
        }
    }
}

impl<A: crate::auth::Authenticator> AgentHandler<A> {
    /// Creates a new Agent handler wrapping the given session.
    ///
    /// Defaults the host to [`AI_PLATFORM_HOST`].
    #[must_use]
    pub(crate) fn new(inner: Arc<crate::session::Session<A>>) -> Self {
        Self {
            inner,
            host: AI_PLATFORM_HOST.to_owned(),
        }
    }

    /// Overrides the AI Platform host (Government Cloud or testing).
    ///
    /// Defaults to [`AI_PLATFORM_HOST`].
    #[must_use]
    pub fn with_host(mut self, host: impl Into<String>) -> Self {
        self.host = host.into();
        self
    }

    /// Builds a full Agent API URL for the given path suffix.
    fn agent_url(&self, suffix: &str) -> String {
        format!(
            "{}/einstein/ai-agent/v1/{suffix}",
            self.host.trim_end_matches('/')
        )
    }

    /// Starts a new agent session.
    ///
    /// `POST /einstein/ai-agent/v1/agents/{agent_id}/sessions`
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response cannot be deserialized.
    pub async fn start_session(
        &self,
        agent_id: &str,
        request: &StartSessionRequest,
    ) -> Result<StartSessionResponse> {
        let url = self.agent_url(&format!("agents/{agent_id}/sessions"));
        let http_request = self
            .inner
            .post(&url)
            .json(request)
            .build()
            .map_err(HttpError::from)?;
        self.inner
            .send_request_and_decode(http_request, "Agent start_session failed")
            .await
    }

    /// Starts a new agent session with sensible defaults.
    ///
    /// Builds a [`StartSessionRequest`] using the org's instance URL for
    /// `instanceConfig.endpoint`, a fresh random `externalSessionKey`, `Text`
    /// streaming capability, and `bypassUser = true`.
    ///
    /// # Errors
    ///
    /// Returns an error if the instance URL cannot be resolved, the request
    /// fails, or the response cannot be deserialized.
    pub async fn start_session_default(&self, agent_id: &str) -> Result<StartSessionResponse> {
        let endpoint = self.inner.instance_url().await?;
        let external_key = uuid::Uuid::new_v4().to_string();
        let request = StartSessionRequest::new(external_key, endpoint).with_bypass_user(true);
        self.start_session(agent_id, &request).await
    }

    /// Sends a synchronous message to an agent session.
    ///
    /// `POST /einstein/ai-agent/v1/sessions/{session_id}/messages`
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response cannot be deserialized.
    pub async fn send_message(
        &self,
        session_id: &str,
        request: &SendMessageRequest,
    ) -> Result<AgentResponse> {
        let url = self.agent_url(&format!("sessions/{session_id}/messages"));
        let http_request = self
            .inner
            .post(&url)
            .json(request)
            .build()
            .map_err(HttpError::from)?;
        self.inner
            .send_request_and_decode(http_request, "Agent send_message failed")
            .await
    }

    /// Sends a plain text message to an agent session.
    ///
    /// Convenience wrapper over [`send_message`](Self::send_message) using
    /// [`SendMessageRequest::text`].
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response cannot be deserialized.
    pub async fn send_text(
        &self,
        session_id: &str,
        sequence_id: i64,
        text: &str,
    ) -> Result<AgentResponse> {
        let request = SendMessageRequest::text(sequence_id, text);
        self.send_message(session_id, &request).await
    }

    /// Ends an agent session.
    ///
    /// `DELETE /einstein/ai-agent/v1/sessions/{session_id}` with the required
    /// `x-session-end-reason` header. Expects a `204 No Content` response.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or returns a non-success status.
    pub async fn end_session(&self, session_id: &str, reason: SessionEndReason) -> Result<()> {
        let url = self.agent_url(&format!("sessions/{session_id}"));
        let http_request = self
            .inner
            .delete(&url)
            .header(SESSION_END_REASON_HEADER, reason.as_str())
            .build()
            .map_err(HttpError::from)?;
        self.inner
            .execute_and_check_success(http_request, "Agent end_session failed")
            .await?;
        Ok(())
    }
}

#[cfg(all(test, feature = "mock"))]
mod tests {
    use super::*;
    use crate::test_utils::mock_auth::MockAuthenticator;
    use crate::test_utils::must::Must;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn setup() -> (MockServer, crate::client::ForceClient<MockAuthenticator>) {
        let server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &server.uri());
        let client = crate::client::builder()
            .authenticate(auth)
            .build()
            .await
            .must();
        (server, client)
    }

    #[tokio::test]
    async fn test_handler_is_cloneable_and_debug() {
        let (_server, client) = setup().await;
        let handler = client.agents();
        let cloned = handler.clone();
        assert_eq!(format!("{handler:?}"), format!("{cloned:?}"));
        assert!(format!("{handler:?}").contains("AgentHandler"));
    }

    #[tokio::test]
    async fn test_start_session_success() {
        let (server, client) = setup().await;

        Mock::given(method("POST"))
            .and(path(
                "/einstein/ai-agent/v1/agents/0XxHr000000ysOSKAY/sessions",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "sessionId": "80ab2e16-392e-4c12-b80a-f028a58400b5",
                "_links": {"messages": {"href": "https://x/messages"}},
                "messages": [
                    {"type": "Inform", "id": "d27b", "message": "Hi, I'm an AI assistant. How can I help?"}
                ]
            })))
            .expect(1)
            .mount(&server)
            .await;

        let req = StartSessionRequest::new("uuid-abc", "https://my.salesforce.com");
        let resp = client
            .agents()
            .with_host(server.uri())
            .start_session("0XxHr000000ysOSKAY", &req)
            .await
            .must();
        assert_eq!(resp.session_id, "80ab2e16-392e-4c12-b80a-f028a58400b5");
        assert_eq!(
            resp.messages[0].message.as_deref(),
            Some("Hi, I'm an AI assistant. How can I help?")
        );
    }

    #[tokio::test]
    async fn test_send_text_success() {
        let (server, client) = setup().await;

        Mock::given(method("POST"))
            .and(path("/einstein/ai-agent/v1/sessions/session-1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "messages": [
                    {"type": "Inform", "message": "To reset your password, go to Settings."}
                ],
                "_links": {}
            })))
            .expect(1)
            .mount(&server)
            .await;

        let resp = client
            .agents()
            .with_host(server.uri())
            .send_text("session-1", 1, "How do I reset my password?")
            .await
            .must();
        assert_eq!(
            resp.reply_text(),
            Some("To reset your password, go to Settings.")
        );
    }

    #[tokio::test]
    async fn test_end_session_success() {
        let (server, client) = setup().await;

        Mock::given(method("DELETE"))
            .and(path("/einstein/ai-agent/v1/sessions/session-1"))
            .and(header("x-session-end-reason", "UserRequest"))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;

        let result = client
            .agents()
            .with_host(server.uri())
            .end_session("session-1", SessionEndReason::UserRequest)
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_send_text_error() {
        let (server, client) = setup().await;

        Mock::given(method("POST"))
            .and(path("/einstein/ai-agent/v1/sessions/session-err/messages"))
            .respond_with(
                ResponseTemplate::new(500)
                    .set_body_json(serde_json::json!({"message": "Internal error"})),
            )
            .mount(&server)
            .await;

        let result = client
            .agents()
            .with_host(server.uri())
            .send_text("session-err", 1, "hi")
            .await;
        assert!(result.is_err());
    }
}
