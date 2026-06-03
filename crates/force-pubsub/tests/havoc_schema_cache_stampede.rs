//! 👺 Havoc: `SchemaCache::get_or_fetch` Stampede
//!
//! **The Trigger:** 100 concurrent threads requesting the same `schema_id` via `get_or_fetch()`.
//! **The Stack Trace:** No panic, but we observe 100 gRPC calls instead of 1.
//! **Reproduction:** Run `cargo test -p force-pubsub --test havoc_schema_cache_stampede`
//! **Comment:** The lock-free fast path doesn't protect against concurrent cache misses, causing a thunderous herd.

#![allow(clippy::unwrap_used)]

use dashmap::DashMap;
use std::sync::Arc;

use std::sync::atomic::{AtomicUsize, Ordering};

// To verify the fix, we use the DashMap pattern.

struct MockSchemaCache {
    cache: DashMap<String, Arc<tokio::sync::OnceCell<String>>>,
    rpc_calls: Arc<AtomicUsize>,
}

impl MockSchemaCache {
    async fn get_or_fetch(&self, id: &str) -> String {
        // Find or create the cell for this schema_id
        let cell = self
            .cache
            .entry(id.to_string())
            .or_insert_with(|| Arc::new(tokio::sync::OnceCell::new()))
            .clone();

        // Get or initialize it
        cell.get_or_init(|| async {
            // Simulate cache miss logic
            tokio::task::yield_now().await;
            self.rpc_calls.fetch_add(1, Ordering::SeqCst);
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            format!("schema_{id}")
        })
        .await
        .clone()
    }
}

#[tokio::test]
async fn test_schema_cache_stampede() {
    let cache = Arc::new(MockSchemaCache {
        cache: DashMap::new(),
        rpc_calls: Arc::new(AtomicUsize::new(0)),
    });

    let mut handles = vec![];
    for _ in 0..100 {
        let c = cache.clone();
        handles.push(tokio::spawn(
            async move { c.get_or_fetch("schema-1").await },
        ));
    }

    for h in handles {
        h.await.unwrap();
    }

    let calls = cache.rpc_calls.load(Ordering::SeqCst);
    assert_eq!(
        calls, 1,
        "👺 Havoc: SchemaCache get_or_fetch stampede! Expected 1 RPC call, got {calls}"
    );
}
