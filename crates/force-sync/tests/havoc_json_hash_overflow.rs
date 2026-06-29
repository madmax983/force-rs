//! 👺 Havoc: JSON Hash Stack Overflow `DoS`

#![allow(clippy::unwrap_used)]

use chrono::Utc;
use force_sync::ChangeEnvelope;
use force_sync::SyncKey;
use force_sync::{ChangeOperation, SourceSystem};
use serde_json::Value;

#[test]
fn test_havoc_json_hash_overflow() {
    let handle = std::thread::Builder::new()
        .stack_size(2 * 1024 * 1024)
        .spawn(|| {
            let mut value = Value::Null;
            // Over 10k! Iterative handles this perfectly.
            for _ in 0..10_000 {
                value = Value::Array(vec![value]);
            }

            let envelope = std::mem::ManuallyDrop::new(ChangeEnvelope::new(
                SyncKey::new("t", "A", "id").unwrap(),
                SourceSystem::Salesforce,
                ChangeOperation::Upsert,
                Utc::now(),
                value,
            ));

            let _r = envelope.payload_hash();
        })
        .unwrap();

    let res = handle.join();
    assert!(
        res.is_ok(),
        "Expected thread to run successfully without aborting."
    );
}
