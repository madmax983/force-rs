//! Change envelope and cursor types for sync events.

use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::identity::SyncKey;

/// Origin system for a captured change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceSystem {
    /// Change originated from Salesforce.
    Salesforce,
    /// Change originated from Postgres.
    Postgres,
}

impl SourceSystem {
    /// Returns the database representation used by the sync journal.
    #[must_use]
    pub const fn as_db_value(self) -> &'static str {
        match self {
            Self::Salesforce => "salesforce",
            Self::Postgres => "postgres",
        }
    }
}

/// Mutation type carried by a change envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeOperation {
    /// Create or update semantics.
    Upsert,
    /// Delete semantics.
    Delete,
}

impl ChangeOperation {
    /// Returns the database representation used by the sync journal.
    #[must_use]
    pub const fn as_db_value(self) -> &'static str {
        match self {
            Self::Upsert => "upsert",
            Self::Delete => "delete",
        }
    }
}

/// Source-specific cursor for replayable capture streams.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceCursor {
    /// Salesforce CDC replay identifier.
    SalesforceReplayId(i64),
    /// Postgres logical sequence number or outbox position.
    PostgresLsn(String),
    /// Snapshot or backfill watermark.
    Snapshot(String),
}

impl SourceCursor {
    /// Returns the database representation used by the sync journal.
    #[must_use]
    pub fn as_db_value(&self) -> String {
        use std::fmt::Write;
        match self {
            Self::SalesforceReplayId(replay_id) => {
                let mut s = String::with_capacity(22 + 20); // prefix + max digits for i64
                let _ = write!(s, "salesforce-replay-id:{replay_id}");
                s
            }
            Self::PostgresLsn(lsn) => {
                let mut s = String::with_capacity(13 + lsn.len());
                s.push_str("postgres-lsn:");
                s.push_str(lsn);
                s
            }
            Self::Snapshot(watermark) => {
                let mut s = String::with_capacity(9 + watermark.len());
                s.push_str("snapshot:");
                s.push_str(watermark);
                s
            }
        }
    }
}

/// Canonical sync event stored in the journal and passed to planners.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeEnvelope {
    sync_key: SyncKey,
    source: SourceSystem,
    operation: ChangeOperation,
    cursor: Option<SourceCursor>,
    observed_at: DateTime<Utc>,
    payload: Value,
}

impl ChangeEnvelope {
    /// Creates a new change envelope with no replay cursor.
    #[must_use]
    pub const fn new(
        sync_key: SyncKey,
        source: SourceSystem,
        operation: ChangeOperation,
        observed_at: DateTime<Utc>,
        payload: Value,
    ) -> Self {
        Self {
            sync_key,
            source,
            operation,
            cursor: None,
            observed_at,
            payload,
        }
    }

    /// Attaches a replay cursor to the envelope.
    #[must_use]
    pub fn with_cursor(mut self, cursor: SourceCursor) -> Self {
        self.cursor = Some(cursor);
        self
    }

    /// Returns the canonical record identity.
    #[must_use]
    pub const fn sync_key(&self) -> &SyncKey {
        &self.sync_key
    }

    /// Returns the source system for the change.
    #[must_use]
    pub const fn source(&self) -> SourceSystem {
        self.source
    }

    /// Returns the operation carried by the change.
    #[must_use]
    pub const fn operation(&self) -> ChangeOperation {
        self.operation
    }

    /// Returns the source cursor, if available.
    #[must_use]
    pub const fn cursor(&self) -> Option<&SourceCursor> {
        self.cursor.as_ref()
    }

    /// Returns when the change was observed by the sync engine.
    #[must_use]
    pub const fn observed_at(&self) -> DateTime<Utc> {
        self.observed_at
    }

    /// Returns the raw change payload.
    #[must_use]
    pub const fn payload(&self) -> &Value {
        &self.payload
    }

    /// Returns a stable hash for the payload contents.
    #[allow(clippy::missing_errors_doc)]
    pub fn payload_hash(&self) -> Result<[u8; 32], crate::error::ForceSyncError> {
        payload_hash(&self.payload)
    }

    /// Returns `true` when the provided payload hashes to the same value.
    #[must_use]
    pub fn payload_hash_matches(&self, other: &Value) -> bool {
        self.payload_hash().is_ok_and(|h| payload_hash(other).is_ok_and(|oh| h == oh))
    }
}

/// Returns a stable BLAKE3 hash for a JSON payload.
#[allow(clippy::missing_errors_doc)]
pub fn payload_hash(payload: &Value) -> Result<[u8; 32], crate::error::ForceSyncError> {
    let mut hasher = blake3::Hasher::new();
    // ⚡ Bolt: Write JSON directly into the hasher without an intermediate String allocation.
    // We avoid `payload.clone()` by manually sorting object keys dynamically during hashing.
    hash_json_value(payload, &mut hasher, 0)?;
    Ok(*hasher.finalize().as_bytes())
}

use std::io::Write;
fn hash_json_value(
    value: &Value,
    hasher: &mut blake3::Hasher,
    depth: usize,
) -> Result<(), crate::error::ForceSyncError> {
    if depth > 128 {
        return Err(crate::error::ForceSyncError::PayloadDepthExceeded);
    }
    match value {
        Value::Object(map) => {
            let _ = Write::write_all(hasher, b"{");
            // ⚡ Bolt: Provide a capacity hint for the intermediate vector to avoid reallocations
            let mut iter = Vec::with_capacity(map.len());
            iter.extend(map.iter());
            iter.sort_unstable_by_key(|(k, _)| *k);
            let mut first = true;
            for (k, v) in iter {
                if !first {
                    let _ = Write::write_all(hasher, b",");
                }
                first = false;
                let _ = serde_json::to_writer(&mut *hasher, k);
                let _ = Write::write_all(hasher, b":");
                hash_json_value(v, hasher, depth + 1)?;
            }
            let _ = Write::write_all(hasher, b"}");
        }
        Value::Array(arr) => {
            let _ = Write::write_all(hasher, b"[");
            let mut first = true;
            for v in arr {
                if !first {
                    let _ = Write::write_all(hasher, b",");
                }
                first = false;
                hash_json_value(v, hasher, depth + 1)?;
            }
            let _ = Write::write_all(hasher, b"]");
        }
        _ => {
            let _ = serde_json::to_writer(hasher, value);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use serde_json::json;

    use crate::identity::SyncKey;

    use super::{ChangeEnvelope, ChangeOperation, SourceCursor, SourceSystem};

    #[test]
    fn change_envelope_payload_hash_is_stable_for_identical_payloads() {
        let sync_key = match SyncKey::new("tenant", "Account", "abc") {
            Ok(sync_key) => sync_key,
            Err(error) => panic!("unexpected sync key construction error: {error}"),
        };
        let payload = json!({"Name": "Acme"});

        let first = ChangeEnvelope::new(
            sync_key.clone(),
            SourceSystem::Salesforce,
            ChangeOperation::Upsert,
            Utc::now(),
            payload.clone(),
        );
        let second = ChangeEnvelope::new(
            sync_key,
            SourceSystem::Salesforce,
            ChangeOperation::Upsert,
            Utc::now(),
            payload,
        );

        assert_eq!(
            first
                .payload_hash()
                .unwrap_or_else(|_| panic!("hash failed")),
            second
                .payload_hash()
                .unwrap_or_else(|_| panic!("hash failed"))
        );
    }

    #[test]
    fn payload_hash_matches_pre_sorted_payloads() {
        let payload = json!({
            "outer": {
                "zeta": 1,
                "alpha": 2
            },
            "items": [
                {
                    "delta": 4,
                    "beta": 3
                }
            ]
        });
        let mut sorted_payload = payload.clone();
        sorted_payload.sort_all_objects();

        assert_eq!(
            super::payload_hash(&payload).unwrap_or_else(|_| panic!("hash failed")),
            super::payload_hash(&sorted_payload).unwrap_or_else(|_| panic!("hash failed"))
        );
    }

    #[test]
    fn change_envelope_with_cursor_attaches_cursor() {
        let sync_key = match SyncKey::new("tenant", "Account", "abc") {
            Ok(sync_key) => sync_key,
            Err(error) => panic!("unexpected sync key construction error: {error}"),
        };

        let envelope = ChangeEnvelope::new(
            sync_key,
            SourceSystem::Salesforce,
            ChangeOperation::Upsert,
            Utc::now(),
            json!({"Name": "Acme"}),
        )
        .with_cursor(SourceCursor::SalesforceReplayId(42));

        assert!(matches!(
            envelope.cursor(),
            Some(SourceCursor::SalesforceReplayId(42))
        ));
    }

    #[test]
    fn change_envelope_payload_hash_matches_semantically_equal_payloads() {
        let sync_key = match SyncKey::new("tenant", "Account", "abc") {
            Ok(sync_key) => sync_key,
            Err(error) => panic!("unexpected sync key construction error: {error}"),
        };

        let envelope = ChangeEnvelope::new(
            sync_key,
            SourceSystem::Salesforce,
            ChangeOperation::Upsert,
            Utc::now(),
            json!({
                "Name": "Acme",
                "Description": "Keep"
            }),
        );

        assert!(envelope.payload_hash_matches(&json!({
            "Description": "Keep",
            "Name": "Acme"
        })));
    }
}
