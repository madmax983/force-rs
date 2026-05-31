//! DashMap-backed schema cache for Avro schemas fetched from the Pub/Sub API.

use apache_avro::Schema;
use dashmap::DashMap;
use std::sync::Arc;
use tonic::transport::Channel;

use crate::error::{PubSubError, Result};
use crate::proto::eventbus_v1::{SchemaRequest, pub_sub_client::PubSubClient};

/// Fetches and caches Avro schemas by schema ID.
///
/// The cache is lock-free for reads via `DashMap`. Each schema is fetched
/// once from the Pub/Sub `GetSchema` RPC and cached for the lifetime of
/// the handler.
#[derive(Debug, Clone)]
pub struct SchemaCache {
    inner: Arc<SchemaCacheInner>,
}

#[derive(Debug)]
struct SchemaCacheInner {
    /// Wrapped in Arc to prevent expensive deep clones of the Avro schema tree on cache hits.
    cache: DashMap<String, Arc<Schema>>,
}

impl SchemaCache {
    /// Creates a new empty schema cache.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(SchemaCacheInner {
                cache: DashMap::new(),
            }),
        }
    }

    /// Insert a pre-parsed schema directly.
    pub fn insert(&self, schema_id: String, schema: Schema) {
        self.inner.cache.insert(schema_id, Arc::new(schema));
    }

    /// Returns the number of cached schemas.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.cache.len()
    }

    /// True if no schemas are cached.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.cache.is_empty()
    }

    /// Get a schema by ID if it is already in cache.
    #[must_use]
    pub fn get(&self, schema_id: &str) -> Option<Arc<Schema>> {
        self.inner
            .cache
            .get(schema_id)
            .map(|r| Arc::clone(r.value()))
    }

    /// Parse Avro schema JSON and store it in the cache.
    ///
    /// Returns the parsed schema. Called after receiving `schema_json` from `GetSchema`.
    pub fn parse_and_insert(&self, schema_id: String, schema_json: &str) -> Result<Arc<Schema>> {
        let schema = Schema::parse_str(schema_json)
            .map_err(|e| PubSubError::Avro(format!("failed to parse schema {schema_id}: {e}")))?;
        let arc_schema = Arc::new(schema);
        self.inner.cache.insert(schema_id, Arc::clone(&arc_schema));
        Ok(arc_schema)
    }

    /// Return the schema for `schema_id` from cache, or fetch it via the `GetSchema` RPC.
    ///
    /// On a cache miss, calls `GetSchema` using the provided gRPC client and
    /// authentication metadata. The fetched schema is parsed, inserted into the
    /// cache, and returned.
    ///
    /// # Errors
    ///
    /// - [`PubSubError::Transport`] if the `GetSchema` RPC fails (e.g. schema not found).
    /// - [`PubSubError::Avro`] if the returned schema JSON cannot be parsed.
    pub async fn get_or_fetch(
        &self,
        schema_id: &str,
        channel: &Channel,
        metadata: tonic::metadata::MetadataMap,
    ) -> Result<Arc<Schema>> {
        // Fast path: lock-free cache hit.
        if let Some(schema) = self.get(schema_id) {
            return Ok(schema);
        }

        // Cache miss — call GetSchema RPC.
        let mut req = tonic::Request::new(SchemaRequest {
            schema_id: schema_id.to_string(),
        });
        *req.metadata_mut() = metadata;

        let resp = PubSubClient::new(channel.clone())
            .get_schema(req)
            .await?
            .into_inner();

        self.parse_and_insert(resp.schema_id, &resp.schema_json)
    }
}

impl Default for SchemaCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIMPLE_SCHEMA_JSON: &str = r#"
    {
        "type": "record",
        "name": "OrderEvent",
        "fields": [
            {"name": "order_id", "type": "string"},
            {"name": "total", "type": "double"}
        ]
    }
    "#;

    #[test]
    fn test_new_cache_is_empty() {
        let cache = SchemaCache::new();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn test_parse_and_insert() {
        let cache = SchemaCache::new();
        let Ok(_) = cache.parse_and_insert("schema-001".to_string(), SIMPLE_SCHEMA_JSON) else {
            panic!("valid schema JSON")
        };
        assert_eq!(cache.len(), 1);
        assert!(!cache.is_empty());
        assert!(cache.get("schema-001").is_some());
    }

    #[test]
    fn test_get_miss_returns_none() {
        let cache = SchemaCache::new();
        assert!(cache.get("nonexistent").is_none());
    }

    #[test]
    fn test_parse_invalid_schema_returns_error() {
        let cache = SchemaCache::new();
        let result = cache.parse_and_insert("bad".to_string(), "{ not valid avro }");
        let Err(err) = result else {
            panic!("Expected an error");
        };
        assert!(matches!(err, PubSubError::Avro(_)));
        assert!(cache.is_empty());
    }

    #[test]
    fn test_cache_is_cloneable_and_shared() {
        let cache = SchemaCache::new();
        let cache2 = cache.clone();

        let Ok(_) = cache.parse_and_insert("shared".to_string(), SIMPLE_SCHEMA_JSON) else {
            panic!("valid")
        };

        // Clone shares the same underlying DashMap via Arc
        assert_eq!(cache2.len(), 1);
        assert!(cache2.get("shared").is_some());
    }

    #[test]
    fn test_default_is_empty() {
        let cache = SchemaCache::default();
        assert!(cache.is_empty());
    }
}
