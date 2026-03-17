//! Pub/Sub API handler.

use std::pin::Pin;
use std::sync::Arc;
use tonic::transport::Channel;

use force::auth::Authenticator;
use force::session::Session;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use tokio_stream::Stream;

use crate::config::{PubSubConfig, ReplayPreset};
use crate::error::{PubSubError, Result};
use crate::publisher::publish_unary;
use crate::schema_cache::SchemaCache;
use crate::subscriber::{subscribe_dynamic, subscribe_typed_dynamic};
use crate::types::{PublishResponse, PubSubEvent};

use crate::proto::eventbus_v1::{
    pub_sub_client::PubSubClient, SchemaRequest, TopicRequest,
};

/// Public-facing topic metadata (mirrors proto without leaking generated types).
#[derive(Debug, Clone)]
pub struct TopicInfo {
    /// Topic name (e.g., `/event/MyEvent__e`).
    pub topic_name: String,
    /// Topic URI.
    pub topic_uri: String,
    /// Whether events can be published to this topic.
    pub can_publish: bool,
    /// Whether events can be subscribed to on this topic.
    pub can_subscribe: bool,
    /// Current Avro schema ID for this topic.
    pub schema_id: String,
}

/// Public-facing schema metadata.
#[derive(Debug, Clone)]
pub struct SchemaInfo {
    /// Schema ID.
    pub schema_id: String,
    /// Avro schema JSON string.
    pub schema_json: String,
}

/// Entry point for all Salesforce Pub/Sub operations.
///
/// Obtained by calling [`PubSubHandler::connect`] with a [`Session`] and [`PubSubConfig`].
/// The handler is cheaply cloneable — clones share the same gRPC channel and schema cache.
#[derive(Clone)]
pub struct PubSubHandler<A: Authenticator> {
    pub(crate) session: Arc<Session<A>>,
    /// Configuration for this handler, used by subscribe/publish operations in later tasks.
    pub(crate) config: PubSubConfig,
    /// Shared schema cache, populated during subscribe/publish operations in later tasks.
    pub schema_cache: SchemaCache,
    pub(crate) channel: Channel,
}

impl<A: Authenticator> PubSubHandler<A> {
    /// Connect to the Pub/Sub gRPC endpoint and return a handler.
    ///
    /// Validates configuration and establishes the gRPC channel. This is async
    /// because channel creation involves a DNS lookup and TLS handshake.
    ///
    /// # Errors
    ///
    /// Returns `PubSubError::Config` if `batch_size` is out of range (1–100).
    /// Returns `PubSubError::Connect` if the gRPC channel cannot be established.
    pub async fn connect(session: Arc<Session<A>>, config: PubSubConfig) -> Result<Self> {
        if config.batch_size < 1 || config.batch_size > 100 {
            return Err(PubSubError::Config(
                "batch_size must be between 1 and 100".to_string(),
            ));
        }

        let channel = Channel::from_shared(config.endpoint.clone())
            .map_err(|e| PubSubError::Config(format!("invalid endpoint: {e}")))?
            .connect()
            .await?;

        Ok(Self {
            session,
            config,
            schema_cache: SchemaCache::new(),
            channel,
        })
    }

    /// Build a gRPC client for each call (channels are cheap to clone).
    fn grpc_client(&self) -> PubSubClient<Channel> {
        PubSubClient::new(self.channel.clone())
    }

    /// Inject auth headers into a tonic request (pre-fetch pattern).
    async fn auth_request<T>(&self, message: T) -> Result<tonic::Request<T>> {
        let token = self.session.token_manager().token().await?;
        let mut req = tonic::Request::new(message);
        let metadata = req.metadata_mut();
        metadata.insert(
            "accesstoken",
            token
                .as_str()
                .parse()
                .map_err(|_| PubSubError::Config("invalid token characters".to_string()))?,
        );
        metadata.insert(
            "instanceurl",
            token
                .instance_url()
                .parse()
                .map_err(|_| PubSubError::Config("invalid instance URL characters".to_string()))?,
        );
        Ok(req)
    }

    /// Fetch metadata about a Pub/Sub topic.
    ///
    /// # Errors
    ///
    /// Returns `PubSubError::Transport` if the gRPC call fails.
    pub async fn get_topic(&self, topic_name: &str) -> Result<TopicInfo> {
        let req = self
            .auth_request(TopicRequest {
                topic_name: topic_name.to_string(),
            })
            .await?;

        let resp = self.grpc_client().get_topic(req).await?;
        let info = resp.into_inner();
        Ok(TopicInfo {
            topic_name: info.topic_name,
            topic_uri: info.topic_uri,
            can_publish: info.can_publish,
            can_subscribe: info.can_subscribe,
            schema_id: info.schema_id,
        })
    }

    /// Fetch an Avro schema by its ID.
    ///
    /// Results are **not** automatically cached here — call [`SchemaCache::parse_and_insert`]
    /// with the returned `schema_json` to cache it.
    ///
    /// # Errors
    ///
    /// Returns `PubSubError::Transport` if the gRPC call fails (including schema not found).
    pub async fn get_schema(&self, schema_id: &str) -> Result<SchemaInfo> {
        let req = self
            .auth_request(SchemaRequest {
                schema_id: schema_id.to_string(),
            })
            .await?;

        let resp = self.grpc_client().get_schema(req).await?;
        let info = resp.into_inner();
        Ok(SchemaInfo {
            schema_id: info.schema_id,
            schema_json: info.schema_json,
        })
    }
}

impl<A: Authenticator + Send + Sync + 'static> PubSubHandler<A> {
    /// Publish events to a topic via the unary Publish RPC.
    ///
    /// `schema_id` must be pre-loaded in the schema cache via `get_schema()`.
    /// Events are Avro-encoded using the cached schema.
    ///
    /// # Errors
    ///
    /// Returns `PubSubError::SchemaNotFound` if the schema is not in the cache.
    /// Returns `PubSubError::Avro` if encoding fails.
    /// Returns `PubSubError::Transport` if the gRPC call fails.
    pub async fn publish<T: Serialize + Send>(
        &self,
        schema_id: &str,
        topic: &str,
        events: Vec<T>,
    ) -> Result<PublishResponse> {
        publish_unary(
            &self.session,
            &self.channel,
            &self.schema_cache,
            schema_id,
            topic,
            events,
        )
        .await
    }

    /// Subscribe to a topic, yielding decoded events as [`serde_json::Value`].
    ///
    /// The returned stream emits [`PubSubEvent<Value>`] items. Use [`ReplayPreset`]
    /// to control where playback starts.
    ///
    /// # Errors
    ///
    /// This method is infallible at call time; errors surface as stream items.
    #[allow(clippy::unused_async)]
    pub async fn subscribe(
        &self,
        topic: &str,
        replay: ReplayPreset,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<PubSubEvent<Value>>> + Send>>> {
        Ok(subscribe_dynamic(
            Arc::clone(&self.session),
            self.config.clone(),
            self.schema_cache.clone(),
            self.channel.clone(),
            topic.to_string(),
            replay,
        ))
    }

    /// Subscribe to a topic, yielding typed events deserialized as `T`.
    ///
    /// # Errors
    ///
    /// This method is infallible at call time; errors surface as stream items.
    #[allow(clippy::unused_async)]
    pub async fn subscribe_typed<T>(
        &self,
        topic: &str,
        replay: ReplayPreset,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<PubSubEvent<T>>> + Send>>>
    where
        T: DeserializeOwned + Send + 'static,
    {
        Ok(subscribe_typed_dynamic(
            Arc::clone(&self.session),
            self.config.clone(),
            self.schema_cache.clone(),
            self.channel.clone(),
            topic.to_string(),
            replay,
        ))
    }
}
