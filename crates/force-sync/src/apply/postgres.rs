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
    use crate::model::{ChangeEnvelope, ChangeOperation, SourceCursor, SourceSystem};
    use chrono::Utc;
    use force::types::SalesforceId;
    use serde_json::json;

    fn test_envelope() -> ChangeEnvelope {
        let sync_key = SyncKey::new("tenant-1", "Account", "ext-1")
            .unwrap_or_else(|e| panic!("failed to create SyncKey: {e}"));

        ChangeEnvelope::new(
            sync_key,
            SourceSystem::Postgres,
            ChangeOperation::Upsert,
            Utc::now(),
            json!({"Name": "Test"}),
        )
        .with_cursor(SourceCursor::PostgresLsn("lsn-1".to_owned()))
    }

    #[test]
    fn test_project_sync_link_without_existing() {
        let env = test_envelope();
        let sf_id = SalesforceId::new("001000000000001AAA")
            .unwrap_or_else(|e| panic!("failed to create SalesforceId: {e}"));

        let result = project_sync_link(None, &env, Some(&sf_id), false);

        assert_eq!(result.tenant, "tenant-1");
        assert_eq!(result.object_name, "Account");
        assert_eq!(result.external_id, "ext-1");
        assert_eq!(result.salesforce_id, Some("001000000000001AAA".to_owned()));
        assert_eq!(result.postgres_id, None);
        assert_eq!(
            result.last_source,
            Some(env.source().as_db_value().to_owned())
        );
        assert_eq!(
            result.last_source_cursor,
            env.cursor().map(crate::model::SourceCursor::as_db_value)
        );
        assert_eq!(result.last_payload_hash, Some(env.payload_hash().to_vec()));
        assert!(!result.tombstone);
    }

    #[test]
    fn test_project_sync_link_with_existing_and_no_sf_id() {
        let env = test_envelope();
        let existing = SyncLink {
            tenant: "tenant-1".to_owned(),
            object_name: "Account".to_owned(),
            external_id: "ext-1".to_owned(),
            salesforce_id: Some("001000000000001AAA".to_owned()),
            postgres_id: Some("pg-1".to_owned()),
            last_source: Some(SourceSystem::Salesforce.as_db_value().to_owned()),
            last_source_cursor: Some(SourceCursor::SalesforceReplayId(100).as_db_value()),
            last_payload_hash: Some(vec![1, 2, 3]),
            tombstone: false,
        };

        let result = project_sync_link(Some(&existing), &env, None, true);

        assert_eq!(result.salesforce_id, Some("001000000000001AAA".to_owned()));
        assert_eq!(result.postgres_id, Some("pg-1".to_owned()));
        assert_eq!(
            result.last_source,
            Some(env.source().as_db_value().to_owned())
        );
        assert_eq!(
            result.last_source_cursor,
            env.cursor().map(crate::model::SourceCursor::as_db_value)
        );
        assert!(result.tombstone);
    }
}
