//! Salesforce Models API handler (Einstein LLM gateway).
//!
//! The Models API exposes Salesforce-hosted large language models for text
//! generation, chat completion, and embeddings. Unlike the org data APIs, it
//! targets the fixed host [`AI_PLATFORM_HOST`] (`https://api.salesforce.com`)
//! with a version-less path (`/einstein/platform/v1/`), not the org
//! `instance_url`.
//!
//! # Feature Flag
//!
//! This module requires the `models` feature flag:
//! ```toml
//! [dependencies]
//! force = { version = "...", features = ["models"] }
//! ```
//!
//! # Authentication
//!
//! The Models API reuses the org's standard OAuth 2.0 bearer token (for example
//! the client-credentials flow). No token exchange is required. The backing
//! OAuth client must be registered as an **External Client App** (not a classic
//! connected app) with the Einstein/Models platform scopes enabled — the wire
//! auth flow is otherwise identical to the other handlers.
//!
//! # Government Cloud
//!
//! Override the host with [`ModelsHandler::with_host`] (for example
//! `https://api.gov.salesforce.com`).
//!
//! # Usage
//!
//! ```ignore
//! use force::api::models::{GenerateTextRequest, ModelName};
//!
//! let client = builder().authenticate(auth).build().await?;
//! let resp = client
//!     .models()
//!     .generate_text(
//!         ModelName::DEFAULT_GPT4_OMNI,
//!         &GenerateTextRequest::new("Invent 3 fun names for donuts"),
//!     )
//!     .await?;
//! println!("{:?}", resp.text());
//! ```

pub(crate) mod types;

pub use types::{
    ChatGenerationRequest, ChatGenerationResponse, ChatMessage, EmbeddingRequest,
    EmbeddingResponse, EmbeddingVector, GenerateTextRequest, GenerateTextResponse, Generation,
    GenerationDetails, ModelName,
};

use crate::error::{HttpError, Result};
use std::sync::Arc;

/// Fixed host for the Einstein AI Platform APIs.
///
/// Override per-handler via [`ModelsHandler::with_host`] for Government Cloud
/// (`https://api.gov.salesforce.com`) or testing.
pub const AI_PLATFORM_HOST: &str = "https://api.salesforce.com";

/// Required app-context header name for every Models request.
const APP_CONTEXT_HEADER: &str = "x-sfdc-app-context";
/// Required app-context header value.
const APP_CONTEXT_VALUE: &str = "EinsteinGPT";
/// Required client-feature-id header name for every Models request.
const FEATURE_ID_HEADER: &str = "x-client-feature-id";
/// Required client-feature-id header value.
const FEATURE_ID_VALUE: &str = "ai-platform-models-connected-app";

/// Salesforce Models API handler.
///
/// Provides typed access to the Einstein LLM gateway (text generation, chat
/// generation, and embeddings) on [`AI_PLATFORM_HOST`].
///
/// The backing OAuth client must be an **External Client App** with the
/// Einstein/Models platform scopes enabled.
///
/// Obtain a handler from [`ForceClient::models`](crate::client::ForceClient::models).
#[derive(Debug)]
pub struct ModelsHandler<A: crate::auth::Authenticator> {
    inner: Arc<crate::session::Session<A>>,
    host: String,
}

impl<A: crate::auth::Authenticator> Clone for ModelsHandler<A> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            host: self.host.clone(),
        }
    }
}

impl<A: crate::auth::Authenticator> ModelsHandler<A> {
    /// Creates a new Models handler wrapping the given session.
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

    /// Applies the two required Models headers to a request builder.
    fn with_models_headers(builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        builder
            .header(APP_CONTEXT_HEADER, APP_CONTEXT_VALUE)
            .header(FEATURE_ID_HEADER, FEATURE_ID_VALUE)
    }

    /// Builds a full Models URL for the given model and endpoint suffix.
    fn model_url(&self, model: &str, suffix: &str) -> String {
        format!(
            "{}/einstein/platform/v1/models/{model}/{suffix}",
            self.host.trim_end_matches('/')
        )
    }

    /// Generates a single-turn text completion for the given prompt.
    ///
    /// `POST /einstein/platform/v1/models/{model}/generations`
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response cannot be deserialized.
    pub async fn generate_text(
        &self,
        model: &str,
        request: &GenerateTextRequest,
    ) -> Result<GenerateTextResponse> {
        let url = self.model_url(model, "generations");
        let http_request = Self::with_models_headers(self.inner.post(&url).json(request))
            .build()
            .map_err(HttpError::from)?;
        self.inner
            .send_request_and_decode(http_request, "Models generate_text failed")
            .await
    }

    /// Generates a chat completion from a list of messages.
    ///
    /// `POST /einstein/platform/v1/models/{model}/chat-generations`
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response cannot be deserialized.
    pub async fn generate_chat(
        &self,
        model: &str,
        request: &ChatGenerationRequest,
    ) -> Result<ChatGenerationResponse> {
        let url = self.model_url(model, "chat-generations");
        let http_request = Self::with_models_headers(self.inner.post(&url).json(request))
            .build()
            .map_err(HttpError::from)?;
        self.inner
            .send_request_and_decode(http_request, "Models generate_chat failed")
            .await
    }

    /// Generates embedding vectors for the given input strings.
    ///
    /// `POST /einstein/platform/v1/models/{model}/embeddings`
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response cannot be deserialized.
    pub async fn generate_embeddings(
        &self,
        model: &str,
        request: &EmbeddingRequest,
    ) -> Result<EmbeddingResponse> {
        let url = self.model_url(model, "embeddings");
        let http_request = Self::with_models_headers(self.inner.post(&url).json(request))
            .build()
            .map_err(HttpError::from)?;
        self.inner
            .send_request_and_decode(http_request, "Models generate_embeddings failed")
            .await
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
        let handler = client.models();
        let cloned = handler.clone();
        assert_eq!(format!("{handler:?}"), format!("{cloned:?}"));
        assert!(format!("{handler:?}").contains("ModelsHandler"));
    }

    #[tokio::test]
    async fn test_default_host_is_api_salesforce() {
        let (_server, client) = setup().await;
        let handler = client.models();
        assert_eq!(handler.host, AI_PLATFORM_HOST);
    }

    #[tokio::test]
    async fn test_generate_text_success() {
        let (server, client) = setup().await;

        Mock::given(method("POST"))
            .and(path(
                "/einstein/platform/v1/models/sfdc_ai__DefaultGPT4Omni/generations",
            ))
            .and(header("x-sfdc-app-context", "EinsteinGPT"))
            .and(header(
                "x-client-feature-id",
                "ai-platform-models-connected-app",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "chatcmpl-1",
                "generation": {"id": "g1", "generatedText": "Donut Delight"},
                "prompt": "names",
                "parameters": {"model": "gpt-4o"}
            })))
            .expect(1)
            .mount(&server)
            .await;

        let req = GenerateTextRequest::new("Invent 3 fun names for donuts");
        let resp = client
            .models()
            .with_host(server.uri())
            .generate_text(ModelName::DEFAULT_GPT4_OMNI, &req)
            .await
            .must();
        assert_eq!(resp.text(), Some("Donut Delight"));
    }

    #[tokio::test]
    async fn test_generate_chat_success() {
        let (server, client) = setup().await;

        Mock::given(method("POST"))
            .and(path(
                "/einstein/platform/v1/models/sfdc_ai__DefaultGPT4Omni/chat-generations",
            ))
            .and(header("x-sfdc-app-context", "EinsteinGPT"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "1",
                "generationDetails": {
                    "generations": [{"role": "assistant", "content": "Here is a cherry pie recipe"}],
                    "parameters": {"model": "gpt-4o"}
                }
            })))
            .expect(1)
            .mount(&server)
            .await;

        let req = ChatGenerationRequest::new(vec![
            ChatMessage::system("You are a helpful assistant."),
            ChatMessage::user("Give me a recipe for cherry pie?"),
        ]);
        let resp = client
            .models()
            .with_host(server.uri())
            .generate_chat(ModelName::DEFAULT_GPT4_OMNI, &req)
            .await
            .must();
        assert_eq!(resp.reply_text(), Some("Here is a cherry pie recipe"));
    }

    #[tokio::test]
    async fn test_generate_embeddings_success() {
        let (server, client) = setup().await;

        Mock::given(method("POST"))
            .and(path(
                "/einstein/platform/v1/models/sfdc_ai__DefaultOpenAITextEmbeddingAda_002/embeddings",
            ))
            .and(header(
                "x-client-feature-id",
                "ai-platform-models-connected-app",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "embeddings": [{"embedding": [0.25, -0.5], "index": 0}],
                "parameters": {"model": "text-embedding-ada-002-v2"}
            })))
            .expect(1)
            .mount(&server)
            .await;

        let req = EmbeddingRequest::new(vec!["give yourself a present".to_owned()]);
        let resp = client
            .models()
            .with_host(server.uri())
            .generate_embeddings(ModelName::DEFAULT_OPENAI_TEXT_EMBEDDING_ADA_002, &req)
            .await
            .must();
        assert_eq!(resp.embeddings.len(), 1);
        assert_eq!(resp.embeddings[0].index, 0);
        assert_eq!(resp.embeddings[0].embedding.len(), 2);
    }

    #[tokio::test]
    async fn test_generate_text_error() {
        let (server, client) = setup().await;

        Mock::given(method("POST"))
            .and(path(
                "/einstein/platform/v1/models/sfdc_ai__DefaultGPT4Omni/generations",
            ))
            .respond_with(
                ResponseTemplate::new(500)
                    .set_body_json(serde_json::json!({"message": "Internal error"})),
            )
            .mount(&server)
            .await;

        let req = GenerateTextRequest::new("hello");
        let result = client
            .models()
            .with_host(server.uri())
            .generate_text(ModelName::DEFAULT_GPT4_OMNI, &req)
            .await;
        assert!(result.is_err());
    }
}
