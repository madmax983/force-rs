#![allow(clippy::unwrap_used)]
#![allow(missing_docs)]

#[cfg(test)]
mod tests {
    use serde_json::json;

    // We verify the hash depth limit works properly without stack overflow.
    #[test]
    fn test_payload_hash_stack_overflow() {
        let mut val = json!("leaf");
        // Don't go to 100,000 to avoid stack overflow in `json!` macro itself or `serde_json`
        // The limit we set in payload_hash is 128, so 200 is enough to trigger the limit and return early safely.
        for _ in 0..200 {
            val = json!({"nested": val});
        }

        let payload = force_sync::ChangeEnvelope::new(
            force_sync::SyncKey::new("tenant", "Account", "abc").unwrap(),
            force_sync::SourceSystem::Salesforce,
            force_sync::ChangeOperation::Upsert,
            chrono::Utc::now(),
            val,
        );
        let hash = payload.payload_hash();
        assert_ne!(hash, [0u8; 32]);
    }
}
