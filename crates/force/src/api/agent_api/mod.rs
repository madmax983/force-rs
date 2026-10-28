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
//! [`AgentHandler::send_message_stream`] calls `.../messages/stream` and
//! returns an [`AgentEventStream`] of typed Server-Sent Events. Set
//! `streamingCapabilities.chunkTypes = ["Text"]` on session start to get
//! `TextChunk` events.
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

pub(crate) mod stream;
pub(crate) mod types;

pub use stream::{
    AgentEventStream, AgentStreamEvent, AgentStreamEventKind, DEFAULT_MAX_EVENT_BYTES, SseDecoder,
    SseEvent,
};
pub use types::{
    AgentMessage, AgentResponse, AgentVariable, InstanceConfig, OutboundMessage,
    SendMessageRequest, SessionEndReason, StartSessionRequest, StartSessionResponse,
    StreamingCapabilities,
};

use crate::error::{HttpError, Result};
use std::sync::Arc;
use std::time::Duration;

/// Default time limit for one streamed turn (headers and body).
pub const DEFAULT_STREAM_TIMEOUT: Duration = Duration::from_mins(5);

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
    stream_timeout: Duration,
}

impl<A: crate::auth::Authenticator> Clone for AgentHandler<A> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            host: self.host.clone(),
            stream_timeout: self.stream_timeout,
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
            stream_timeout: DEFAULT_STREAM_TIMEOUT,
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

    /// Sets the time limit for one streamed turn (headers and body).
    ///
    /// The client `timeout` applies to the full body, which is too short for
    /// long streams. Defaults to [`DEFAULT_STREAM_TIMEOUT`].
    #[must_use]
    pub const fn with_stream_timeout(mut self, timeout: Duration) -> Self {
        self.stream_timeout = timeout;
        self
    }

    /// Returns the time limit for one streamed turn.
    #[must_use]
    pub const fn stream_timeout(&self) -> Duration {
        self.stream_timeout
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

    /// Sends a message and streams the reply as Server-Sent Events.
    ///
    /// `POST /einstein/ai-agent/v1/sessions/{session_id}/messages/stream` with
    /// `Accept: text/event-stream`. Events arrive in this order:
    /// `ProgressIndicator`, `TextChunk` (when chunking is on), `Inform`,
    /// `EndOfTurn`. The [`stream_timeout`](Self::stream_timeout) applies.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the status is not success.
    /// Stream items are errors if the body fails or an event is too large.
    pub async fn send_message_stream(
        &self,
        session_id: &str,
        request: &SendMessageRequest,
    ) -> Result<AgentEventStream> {
        let url = self.agent_url(&format!("sessions/{session_id}/messages/stream"));
        let http_request = self
            .inner
            .post(&url)
            .header(reqwest::header::ACCEPT, "text/event-stream")
            .timeout(self.stream_timeout)
            .json(request)
            .build()
            .map_err(HttpError::from)?;
        let response = self
            .inner
            .execute_and_check_success(http_request, "Agent send_message_stream failed")
            .await?;
        Ok(stream::sse_event_stream(
            response.bytes_stream(),
            DEFAULT_MAX_EVENT_BYTES,
        ))
    }

    /// Sends plain text and streams the reply.
    ///
    /// Wraps [`send_message_stream`](Self::send_message_stream).
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the status is not success.
    pub async fn send_text_stream(
        &self,
        session_id: &str,
        sequence_id: i64,
        text: &str,
    ) -> Result<AgentEventStream> {
        let request = SendMessageRequest::text(sequence_id, text);
        self.send_message_stream(session_id, &request).await
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

    const SSE_BODY: &str = "event: PROGRESS_INDICATOR\n\
data: {\"message\":{\"type\":\"ProgressIndicator\",\"message\":\"Working\"}}\n\n\
: keep-alive\n\n\
event: TEXT_CHUNK\n\
data: {\"offset\":0,\"message\":{\"type\":\"TextChunk\",\"message\":\"Hel\"}}\n\n\
event: TEXT_CHUNK\n\
data: {\"offset\":3,\"message\":{\"type\":\"TextChunk\",\"message\":\"lo\"}}\n\n\
event: INFORM\n\
data: {\"message\":{\"type\":\"Inform\",\"id\":\"m1\",\"message\":\"Hello\"}}\n\n\
event: END_OF_TURN\n\
data: {\"message\":{\"type\":\"EndOfTurn\"}}\n\n";

    #[tokio::test]
    async fn test_send_text_stream_yields_events_in_order() {
        use futures::StreamExt;
        let (server, client) = setup().await;

        Mock::given(method("POST"))
            .and(path(
                "/einstein/ai-agent/v1/sessions/session-1/messages/stream",
            ))
            .and(wiremock::matchers::header("accept", "text/event-stream"))
            .and(wiremock::matchers::body_partial_json(serde_json::json!({
                "message": {"sequenceId": 2, "type": "Text", "text": "hi"}
            })))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(SSE_BODY),
            )
            .expect(1)
            .mount(&server)
            .await;

        let stream = client
            .agents()
            .with_host(server.uri())
            .send_text_stream("session-1", 2, "hi")
            .await
            .must();
        let events: Vec<AgentStreamEvent> = stream.map(|e| e.must()).collect().await;
        let kinds: Vec<_> = events.iter().map(|e| e.kind.clone()).collect();
        assert_eq!(
            kinds,
            vec![
                AgentStreamEventKind::ProgressIndicator,
                AgentStreamEventKind::TextChunk,
                AgentStreamEventKind::TextChunk,
                AgentStreamEventKind::Inform,
                AgentStreamEventKind::EndOfTurn,
            ]
        );
        let chunks: String = events
            .iter()
            .filter(|e| e.kind == AgentStreamEventKind::TextChunk)
            .filter_map(AgentStreamEvent::text)
            .collect();
        assert_eq!(chunks, "Hello");
        assert_eq!(events[3].text(), Some("Hello"));
    }

    #[tokio::test]
    async fn test_send_message_stream_http_error() {
        let (server, client) = setup().await;
        Mock::given(method("POST"))
            .and(path("/einstein/ai-agent/v1/sessions/bad/messages/stream"))
            .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!([
                {"errorCode": "NOT_FOUND", "message": "session not found"}
            ])))
            .mount(&server)
            .await;

        let result = client
            .agents()
            .with_host(server.uri())
            .send_text_stream("bad", 1, "hi")
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_sse_event_stream_stops_after_oversize_event() {
        use futures::StreamExt;
        let chunks: Vec<std::result::Result<bytes::Bytes, reqwest::Error>> = vec![
            Ok(bytes::Bytes::from_static(
                b"data: {\"message\":{\"type\":\"Inform\"}}\n\n",
            )),
            Ok(bytes::Bytes::from(format!("data: {}\n\n", "x".repeat(64)))),
            Ok(bytes::Bytes::from_static(b"data: never\n\n")),
        ];
        let stream = stream::sse_event_stream(futures::stream::iter(chunks), 48);
        let results: Vec<_> = stream.collect().await;
        assert_eq!(results.len(), 2);
        assert!(results[0].is_ok());
        assert!(results[1].is_err());
    }

    #[tokio::test]
    async fn test_with_stream_timeout_sets_value() {
        let (_server, client) = setup().await;
        let handler = client.agents();
        assert_eq!(handler.stream_timeout(), DEFAULT_STREAM_TIMEOUT);
        let handler = handler
            .with_host("https://h")
            .with_stream_timeout(std::time::Duration::from_secs(5));
        assert_eq!(handler.stream_timeout(), std::time::Duration::from_secs(5));
        assert!(format!("{handler:?}").contains("https://h"));
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
