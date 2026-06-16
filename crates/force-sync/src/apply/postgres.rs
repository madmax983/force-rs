//! Postgres-side projection helpers for apply results.

use force::types::SalesforceId;

use crate::{model::ChangeEnvelope, store::pg::SyncLink};

/// Projects the latest sync link row after a successful apply.
#[must_use]
pub fn project_sync_link(
    existing: Option<&SyncLink>,
    envelope: &ChangeEnvelope,
    salesforce_id: Option<&SalesforceId>,
    tombstone: bool,
) -> SyncLink {
    SyncLink {
        tenant: envelope.sync_key().tenant().to_owned(),
        object_name: envelope.sync_key().object_name().to_owned(),
        external_id: envelope.sync_key().external_id().to_owned(),
        salesforce_id: salesforce_id
            .map(|id| id.as_str().to_owned())
            .or_else(|| existing.and_then(|link| link.salesforce_id.clone())),
        postgres_id: existing.and_then(|link| link.postgres_id.clone()),
        last_source: Some(envelope.source().as_db_value().to_owned()),
        last_source_cursor: envelope
            .cursor()
            .map(crate::model::SourceCursor::as_db_value),
        last_payload_hash: Some(envelope.payload_hash().to_vec()),
        tombstone,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::SyncKey;
    use crate::model::{ChangeOperation, SourceSystem};
    use chrono::Utc;
    use serde_json::json;

    fn mock_envelope() -> ChangeEnvelope {
        ChangeEnvelope::new(
            SyncKey::new("t1", "Account", "ext1")
                .unwrap_or_else(|_| panic!("failed to create SyncKey")),
            SourceSystem::Postgres,
            ChangeOperation::Upsert,
            Utc::now(),
            json!({"name": "Test"}),
        )
    }

    #[test]
    fn project_sync_link_new_record_no_cursor() {
        let envelope = mock_envelope();
        let link = project_sync_link(None, &envelope, None, false);

        assert_eq!(link.tenant, "t1");
        assert_eq!(link.object_name, "Account");
        assert_eq!(link.external_id, "ext1");
        assert_eq!(link.salesforce_id, None);
        assert_eq!(link.postgres_id, None);
        assert_eq!(link.last_source, Some("postgres".to_owned()));
        assert_eq!(link.last_source_cursor, None);
        assert_eq!(
            link.last_payload_hash,
            Some(envelope.payload_hash().to_vec())
        );
        assert!(!link.tombstone);
    }

    #[test]
    fn project_sync_link_with_existing_and_cursor() {
        let envelope = mock_envelope().with_cursor(crate::model::SourceCursor::PostgresLsn(
            "0/16B3748".to_string(),
        ));
        let existing = SyncLink {
            tenant: "t1".to_string(),
            object_name: "Account".to_string(),
            external_id: "ext1".to_string(),
            salesforce_id: Some("001000000000000".to_string()),
            postgres_id: Some("pg-id-1".to_string()),
            last_source: Some("salesforce".to_string()),
            last_source_cursor: Some("salesforce-replay-id:123".to_string()),
            last_payload_hash: Some(vec![1, 2, 3]),
            tombstone: false,
        };

        let link = project_sync_link(Some(&existing), &envelope, None, true);

        assert_eq!(link.salesforce_id, Some("001000000000000".to_string()));
        assert_eq!(link.postgres_id, Some("pg-id-1".to_string()));
        assert_eq!(link.last_source, Some("postgres".to_string()));
        assert_eq!(
            link.last_source_cursor,
            Some("postgres-lsn:0/16B3748".to_string())
        );
        assert!(link.tombstone);
    }
}
