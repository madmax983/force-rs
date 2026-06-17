//! Test `DoS` protections in JSON hashing.

use force_sync::{ChangeEnvelope, ChangeOperation, SourceSystem, SyncKey};
use serde_json::Value;

#[test]
fn test_hash_json_dos() {
    let mut obj = Value::Null;
    for _ in 0..10_000 {
        let mut map = serde_json::Map::new();
        map.insert("a".to_string(), obj);
        obj = Value::Object(map);
    }

    let sync_key =
        SyncKey::new("tenant", "Account", "abc").unwrap_or_else(|_| panic!("valid sync key"));
    let envelope = ChangeEnvelope::new(
        sync_key,
        SourceSystem::Salesforce,
        ChangeOperation::Upsert,
        chrono::Utc::now(),
        obj,
    );

    // Should not stack overflow from `payload_hash`
    let _hash = envelope.payload_hash();

    // Must leak the envelope to prevent `serde_json::Value::Drop` from stack overflowing
    // The test is verifying `payload_hash` safety, not `Drop` safety (which is out of our control).
    std::mem::forget(envelope);
}
