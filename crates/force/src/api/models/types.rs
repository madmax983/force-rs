//! Request and response types for the Salesforce Models (Einstein LLM) API.
//!
//! Response types intentionally keep Trust Layer and provider-specific fields
//! permissive (`Option<T>` / [`serde_json::Value`]) because the exact shape
//! varies by model provider and evolves over time.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;
use std::ops::Deref;

/// Identifier of a Salesforce-hosted model (e.g. `sfdc_ai__DefaultGPT4Omni`).
///
/// The set of supported models evolves, so this is an open `String` newtype
/// rather than a closed enum. A handful of common models are exposed as
/// associated `&str` constants for discoverability.
///
/// # Examples
///
/// ```
/// use force::api::models::ModelName;
///
/// // Use a well-known constant directly (it is a `&str`).
/// let model: &str = ModelName::DEFAULT_GPT4_OMNI;
/// assert_eq!(model, "sfdc_ai__DefaultGPT4Omni");
///
/// // Or wrap an arbitrary name.
/// let custom = ModelName::from("sfdc_ai__SomeNewModel");
/// assert_eq!(custom.as_str(), "sfdc_ai__SomeNewModel");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ModelName(String);

impl ModelName {
    /// Default GPT-4 Omni chat/text model.
    pub const DEFAULT_GPT4_OMNI: &'static str = "sfdc_ai__DefaultGPT4Omni";
    /// Default GPT-4 Omni Mini chat/text model.
    pub const DEFAULT_GPT4_OMNI_MINI: &'static str = "sfdc_ai__DefaultGPT4OmniMini";
    /// Default OpenAI text embedding (ada-002) model.
    pub const DEFAULT_OPENAI_TEXT_EMBEDDING_ADA_002: &'static str =
        "sfdc_ai__DefaultOpenAITextEmbeddingAda_002";

    /// Creates a new [`ModelName`] from any string-like value.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// Returns the model name as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ModelName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Deref for ModelName {
    type Target = str;

    fn deref(&self) -> &str {
        &self.0
    }
}

impl From<&str> for ModelName {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<String> for ModelName {
    fn from(value: String) -> Self {
        Self(value)
    }
}

// ── Generate Text ────────────────────────────────────────────────────────

/// Request body for the Generate Text endpoint (`/models/{model}/generations`).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateTextRequest {
    /// The prompt to complete.
    pub prompt: String,
    /// Optional localization hints (default locale, input locales, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub localization: Option<Value>,
    /// Optional free-form tags echoed back for tracing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Value>,
}

impl GenerateTextRequest {
    /// Creates a new request with just a prompt.
    #[must_use]
    pub fn new(prompt: impl Into<String>) -> Self {
        Self {
            prompt: prompt.into(),
            localization: None,
            tags: None,
        }
    }

    /// Sets the localization hints.
    #[must_use]
    pub fn with_localization(mut self, localization: Value) -> Self {
        self.localization = Some(localization);
        self
    }

    /// Sets the tracing tags.
    #[must_use]
    pub fn with_tags(mut self, tags: Value) -> Self {
        self.tags = Some(tags);
        self
    }
}

/// A single generation produced by a model.
///
/// Used by both the text (`/generations`) and chat (`/chat-generations`)
/// endpoints; not every field is populated by every endpoint.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Generation {
    /// Provider-assigned generation id.
    #[serde(default)]
    pub id: Option<String>,
    /// The generated text (populated by `/generations`).
    #[serde(default)]
    pub generated_text: Option<String>,
    /// Role of the message (populated by `/chat-generations`, e.g. `assistant`).
    #[serde(default)]
    pub role: Option<String>,
    /// Message content (populated by `/chat-generations`).
    #[serde(default)]
    pub content: Option<String>,
    /// Provider timestamp (epoch seconds), when present.
    #[serde(default)]
    pub timestamp: Option<i64>,
    /// Trust Layer content-quality signals (shape varies; kept permissive).
    #[serde(default)]
    pub content_quality: Option<Value>,
    /// Provider-specific generation parameters (finish reason, index, ...).
    #[serde(default)]
    pub parameters: Option<Value>,
}

/// Response body for the Generate Text endpoint.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateTextResponse {
    /// Provider response id.
    #[serde(default)]
    pub id: Option<String>,
    /// The primary generation.
    #[serde(default)]
    pub generation: Option<Generation>,
    /// The prompt echoed back.
    #[serde(default)]
    pub prompt: Option<String>,
    /// Response-level parameters (model, usage, ...).
    #[serde(default)]
    pub parameters: Option<Value>,
    /// Additional generations, when the model returns more than one.
    #[serde(default)]
    pub more_generations: Option<Vec<Generation>>,
}

impl GenerateTextResponse {
    /// Returns the primary generated text, if present.
    #[must_use]
    pub fn text(&self) -> Option<&str> {
        self.generation
            .as_ref()
            .and_then(|g| g.generated_text.as_deref())
    }
}

// ── Generate Chat ────────────────────────────────────────────────────────

/// A single chat message (`role` + `content`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// The role of the author: `system`, `user`, or `assistant`.
    pub role: String,
    /// The message text.
    pub content: String,
}

impl ChatMessage {
    /// Creates a `system` message.
    #[must_use]
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".to_owned(),
            content: content.into(),
        }
    }

    /// Creates a `user` message.
    #[must_use]
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".to_owned(),
            content: content.into(),
        }
    }

    /// Creates an `assistant` message.
    #[must_use]
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".to_owned(),
            content: content.into(),
        }
    }
}

/// Request body for the Generate Chat endpoint (`/models/{model}/chat-generations`).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatGenerationRequest {
    /// The conversation so far, oldest first.
    pub messages: Vec<ChatMessage>,
    /// Optional localization hints.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub localization: Option<Value>,
    /// Optional tracing tags.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Value>,
}

impl ChatGenerationRequest {
    /// Creates a new chat request from a list of messages.
    #[must_use]
    pub fn new(messages: Vec<ChatMessage>) -> Self {
        Self {
            messages,
            localization: None,
            tags: None,
        }
    }

    /// Sets the localization hints.
    #[must_use]
    pub fn with_localization(mut self, localization: Value) -> Self {
        self.localization = Some(localization);
        self
    }

    /// Sets the tracing tags.
    #[must_use]
    pub fn with_tags(mut self, tags: Value) -> Self {
        self.tags = Some(tags);
        self
    }
}

/// Details block returned by the chat endpoint.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerationDetails {
    /// The generated messages (usually one).
    #[serde(default)]
    pub generations: Vec<Generation>,
    /// Response-level parameters (model, usage, ...).
    #[serde(default)]
    pub parameters: Option<Value>,
}

/// Response body for the Generate Chat endpoint.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatGenerationResponse {
    /// Provider response id.
    #[serde(default)]
    pub id: Option<String>,
    /// The generation details (messages + parameters).
    #[serde(default)]
    pub generation_details: Option<GenerationDetails>,
}

impl ChatGenerationResponse {
    /// Returns the assistant's reply text (the first generation's content).
    #[must_use]
    pub fn reply_text(&self) -> Option<&str> {
        self.generation_details
            .as_ref()
            .and_then(|d| d.generations.first())
            .and_then(|g| g.content.as_deref())
    }
}

// ── Embeddings ───────────────────────────────────────────────────────────

/// Request body for the Embeddings endpoint (`/models/{model}/embeddings`).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingRequest {
    /// One or more strings to embed.
    pub input: Vec<String>,
    /// Optional localization hints.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub localization: Option<Value>,
    /// Optional tracing tags.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Value>,
}

impl EmbeddingRequest {
    /// Creates a new embedding request for the given inputs.
    #[must_use]
    pub fn new(input: Vec<String>) -> Self {
        Self {
            input,
            localization: None,
            tags: None,
        }
    }

    /// Sets the localization hints.
    #[must_use]
    pub fn with_localization(mut self, localization: Value) -> Self {
        self.localization = Some(localization);
        self
    }

    /// Sets the tracing tags.
    #[must_use]
    pub fn with_tags(mut self, tags: Value) -> Self {
        self.tags = Some(tags);
        self
    }
}

/// A single embedding vector and its position in the input list.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingVector {
    /// The embedding values.
    #[serde(default)]
    pub embedding: Vec<f32>,
    /// Zero-based index of the corresponding input string.
    #[serde(default)]
    pub index: usize,
}

/// Response body for the Embeddings endpoint.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingResponse {
    /// One embedding per input string.
    #[serde(default)]
    pub embeddings: Vec<EmbeddingVector>,
    /// Response-level parameters (model, usage, ...).
    #[serde(default)]
    pub parameters: Option<Value>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::must::Must;
    use serde_json::json;

    #[test]
    fn test_model_name_constants_and_accessors() {
        assert_eq!(ModelName::DEFAULT_GPT4_OMNI, "sfdc_ai__DefaultGPT4Omni");
        assert_eq!(
            ModelName::DEFAULT_OPENAI_TEXT_EMBEDDING_ADA_002,
            "sfdc_ai__DefaultOpenAITextEmbeddingAda_002"
        );
        let name = ModelName::from("sfdc_ai__DefaultGPT4Omni");
        assert_eq!(name.as_str(), "sfdc_ai__DefaultGPT4Omni");
        assert_eq!(name.to_string(), "sfdc_ai__DefaultGPT4Omni");
        // Deref to &str.
        assert!(name.starts_with("sfdc_ai__"));
    }

    #[test]
    fn test_generate_text_request_serialization_skips_none() {
        let req = GenerateTextRequest::new("Hello");
        let v = serde_json::to_value(&req).must();
        assert_eq!(v["prompt"], "Hello");
        assert!(v.get("localization").is_none());
        assert!(v.get("tags").is_none());
    }

    #[test]
    fn test_generate_text_response_text_helper() {
        let json_str = r#"{
            "id": "chatcmpl-1",
            "generation": {"id": "g1", "generatedText": "Donut Delight"},
            "prompt": "names",
            "parameters": {"model": "gpt"}
        }"#;
        let resp: GenerateTextResponse = serde_json::from_str(json_str).must();
        assert_eq!(resp.text(), Some("Donut Delight"));
    }

    #[test]
    fn test_chat_message_constructors() {
        assert_eq!(ChatMessage::system("s").role, "system");
        assert_eq!(ChatMessage::user("u").role, "user");
        assert_eq!(ChatMessage::assistant("a").role, "assistant");
    }

    #[test]
    fn test_chat_response_reply_text() {
        let json_str = r#"{
            "id": "1",
            "generationDetails": {
                "generations": [{"role": "assistant", "content": "Here is a recipe"}],
                "parameters": {"model": "gpt"}
            }
        }"#;
        let resp: ChatGenerationResponse = serde_json::from_str(json_str).must();
        assert_eq!(resp.reply_text(), Some("Here is a recipe"));
    }

    #[test]
    fn test_embedding_response_deserialization() {
        let json_str = r#"{
            "embeddings": [{"embedding": [0.1, -0.2], "index": 0}],
            "parameters": {"model": "ada"}
        }"#;
        let resp: EmbeddingResponse = serde_json::from_str(json_str).must();
        assert_eq!(resp.embeddings.len(), 1);
        assert_eq!(resp.embeddings[0].index, 0);
        assert!((resp.embeddings[0].embedding[0] - 0.1).abs() < f32::EPSILON);
    }

    #[test]
    fn test_embedding_request_serialization() {
        let req = EmbeddingRequest::new(vec!["a".to_owned()]).with_tags(json!({}));
        let v = serde_json::to_value(&req).must();
        assert_eq!(v["input"][0], "a");
        assert!(v.get("tags").is_some());
    }
}
