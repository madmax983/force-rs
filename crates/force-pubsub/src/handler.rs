//! Pub/Sub API handler.

use std::pin::Pin;
use std::sync::Arc;
use tonic::transport::{Channel, ClientTlsConfig};

use force::auth::Authenticator;
use force::session::Session;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use tokio::sync::OnceCell;
use tokio_stream::Stream;

use crate::config::{PubSubConfig, ReplayPreset};
use crate::error::{PubSubError, Result};
use crate::interceptor;
use crate::publish_sink::{PublishSink, open_publish_stream};
use crate::publisher::publish_unary;
use crate::schema_cache::SchemaCache;
use crate::subscriber::{subscribe_dynamic, subscribe_typed_dynamic};
use crate::types::{PubSubEvent, PublishResponse};

use crate::proto::eventbus_v1::{SchemaRequest, TopicRequest, pub_sub_client::PubSubClient};

/// JSON structure of Salesforce's `/services/oauth2/userinfo` response (relevant fields only).
#[derive(serde::Deserialize)]
struct UserInfo {
    organization_id: String,
}

/// Fetch the 18-char org ID from the Salesforce userinfo endpoint.
///
/// Used by [`PubSubHandler::get_tenant_id`] as the initialiser for its [`OnceCell`].
async fn fetch_tenant_id<A: Authenticator>(session: &Arc<Session<A>>) -> Result<String> {
    let token = session.token_manager().token().await?;
    let userinfo_url = format!("{}/services/oauth2/userinfo", token.instance_url());

    let resp = reqwest::Client::new()
        .get(&userinfo_url)
        .bearer_auth(token.as_str())
        .send()
        .await
        .map_err(|e| PubSubError::Config(format!("userinfo request failed: {e}")))?;

    if !resp.status().is_success() {
        return Err(PubSubError::Config(format!(
            "userinfo returned status {}",
            resp.status()
        )));
    }

    let body = force::http::read_capped_body(resp, 1024 * 1024)
        .await
        .map_err(|e| PubSubError::Config(format!("userinfo parse failed: {e}")))?;

    let info: UserInfo = serde_json::from_str(&body)
        .map_err(|e| PubSubError::Config(format!("userinfo parse failed: {e}")))?;

    Ok(info.organization_id)
}

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
    /// Lazily fetched org ID (18-char) from `/services/oauth2/userinfo`.
    ///
    /// Populated on the first call to [`Self::get_tenant_id`] and reused thereafter.
    tenant_id: Arc<OnceCell<String>>,
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

        let endpoint = Channel::from_shared(config.endpoint.clone())
            .map_err(|e| PubSubError::Config(format!("invalid endpoint: {e}")))?;
        let endpoint = if endpoint.uri().scheme_str() == Some("https") {
            endpoint.tls_config(ClientTlsConfig::new().with_webpki_roots())?
        } else {
            endpoint
        };
        let channel = endpoint.connect().await?;

        Ok(Self {
            session,
            config,
            schema_cache: SchemaCache::new(),
            channel,
            tenant_id: Arc::new(OnceCell::new()),
        })
    }

    /// Build a gRPC client for each call (channels are cheap to clone).
    fn grpc_client(&self) -> PubSubClient<Channel> {
        PubSubClient::new(self.channel.clone())
    }

    /// Fetch the org's 18-char tenant ID from `/services/oauth2/userinfo`, caching the result.
    ///
    /// Salesforce's userinfo response includes `organization_id` which is the 18-char org ID
    /// required as the `tenantid` gRPC metadata header.
    ///
    /// # Errors
    ///
    /// Returns [`PubSubError::Config`] if the userinfo endpoint cannot be reached or the
    /// response does not contain a valid `organization_id` field.
    pub(crate) async fn get_tenant_id(&self) -> Result<&str> {
        self.tenant_id
            .get_or_try_init(|| fetch_tenant_id(&self.session))
            .await
            .map(String::as_str)
    }

    /// Build a tonic request with all three required Pub/Sub auth headers.
    ///
    /// Fetches a fresh token and (lazily) the tenant ID, then delegates to
    /// [`crate::interceptor::build_metadata`].
    async fn auth_request<T>(&self, message: T) -> Result<tonic::Request<T>> {
        let token = self.session.token_manager().token().await?;
        let tenant_id = self.get_tenant_id().await?.to_string();
        let meta = interceptor::build_metadata(&token, token.instance_url(), &tenant_id)?;
        let mut req = tonic::Request::new(message);
        *req.metadata_mut() = meta;
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
    /// Automatically resolves the Avro schema for `topic` by calling `GetTopic`
    /// to obtain the schema ID, then fetching the schema via `GetSchema` if it
    /// is not already cached.
    ///
    /// # Errors
    ///
    /// Returns `PubSubError::Transport` if the `GetTopic` or `GetSchema` RPC fails.
    /// Returns `PubSubError::Avro` if encoding fails.
    pub async fn publish<T: Serialize + Send>(
        &self,
        topic: &str,
        events: Vec<T>,
    ) -> Result<PublishResponse> {
        let topic_info = self.get_topic(topic).await?;
        let schema_id = &topic_info.schema_id;

        // Ensure the schema is in the cache (fetches from GetSchema if not).
        let token = self.session.token_manager().token().await?;
        let tenant_id = self.get_tenant_id().await?.to_string();
        let meta = interceptor::build_metadata(&token, token.instance_url(), &tenant_id)?;
        self.schema_cache
            .get_or_fetch(schema_id, &self.channel, meta)
            .await?;

        publish_unary(
            &self.session,
            &self.channel,
            &self.schema_cache,
            schema_id,
            topic,
            events,
            &tenant_id,
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
    /// Returns [`PubSubError::Config`] if the tenant ID cannot be fetched from userinfo.
    pub async fn subscribe(
        &self,
        topic: &str,
        replay: ReplayPreset,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<PubSubEvent<Value>>> + Send>>> {
        let tenant_id = self.get_tenant_id().await?.to_string();
        Ok(subscribe_dynamic(
            Arc::clone(&self.session),
            self.config.clone(),
            self.schema_cache.clone(),
            self.channel.clone(),
            topic.to_string(),
            replay,
            tenant_id,
        ))
    }

    /// Subscribe to a topic, yielding typed events deserialized as `T`.
    ///
    /// # Errors
    ///
    /// Returns [`PubSubError::Config`] if the tenant ID cannot be fetched from userinfo.
    pub async fn subscribe_typed<T>(
        &self,
        topic: &str,
        replay: ReplayPreset,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<PubSubEvent<T>>> + Send>>>
    where
        T: DeserializeOwned + Send + 'static,
    {
        let tenant_id = self.get_tenant_id().await?.to_string();
        Ok(subscribe_typed_dynamic(
            Arc::clone(&self.session),
            self.config.clone(),
            self.schema_cache.clone(),
            self.channel.clone(),
            topic.to_string(),
            replay,
            tenant_id,
        ))
    }

    /// Open a bidirectional streaming `PublishStream` RPC and return a [`PublishSink`].
    ///
    /// The returned sink allows callers to send multiple batches of events to
    /// `topic` without the per-call overhead of the unary [`Self::publish`] RPC.
    /// The server streams back [`PublishResponse`] acknowledgements, which are
    /// accessible via [`PublishSink::responses`].
    ///
    /// # Errors
    ///
    /// - [`PubSubError::Config`] if the tenant ID cannot be fetched.
    /// - [`PubSubError::Transport`] if the gRPC stream cannot be opened.
    pub async fn publish_stream<T: Serialize + Send + 'static>(
        &self,
        topic: &str,
    ) -> Result<PublishSink<T>> {
        let token = self.session.token_manager().token().await?;
        let tenant_id = self.get_tenant_id().await?.to_string();

        open_publish_stream(
            Arc::clone(&self.session),
            self.channel.clone(),
            self.schema_cache.clone(),
            tenant_id,
            topic.to_string(),
            &token,
        )
        .await
    }
}
