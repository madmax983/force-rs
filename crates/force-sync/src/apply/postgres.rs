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
    use crate::model::{ChangeEnvelope, ChangeOperation, SourceSystem};
    use chrono::Utc;
    use serde_json::json;

    fn test_envelope() -> ChangeEnvelope {
        let sync_key = SyncKey::new("tenant_a", "Account", "ext_123")
            .unwrap_or_else(|_| panic!("valid sync key"));
        ChangeEnvelope::new(
            sync_key,
            SourceSystem::Postgres,
            ChangeOperation::Upsert,
            Utc::now(),
            json!({"Name": "Test"}),
        )
    }

    #[test]
    fn project_sync_link_handles_none_existing_and_none_sfid() {
        let envelope = test_envelope();
        let link = project_sync_link(None, &envelope, None, false);

        assert_eq!(link.tenant, "tenant_a");
        assert_eq!(link.object_name, "Account");
        assert_eq!(link.external_id, "ext_123");
        assert_eq!(link.salesforce_id, None);
        assert_eq!(link.postgres_id, None);
        assert_eq!(link.last_source, Some("postgres".to_string()));
        assert!(!link.tombstone);
    }

    #[test]
    fn project_sync_link_uses_provided_sfid() {
        let envelope = test_envelope();
        let sfid = SalesforceId::new("001000000000000AAA").unwrap_or_else(|_| panic!("valid sfid"));
        let link = project_sync_link(None, &envelope, Some(&sfid), false);

        assert_eq!(link.salesforce_id, Some("001000000000000AAA".to_string()));
    }

    #[test]
    fn project_sync_link_falls_back_to_existing_sfid() {
        let envelope = test_envelope();
        let existing = SyncLink {
            tenant: "tenant_a".to_string(),
            object_name: "Account".to_string(),
            external_id: "ext_123".to_string(),
            salesforce_id: Some("001000000000000BBB".to_string()),
            postgres_id: Some("pg_1".to_string()),
            last_source: None,
            last_source_cursor: None,
            last_payload_hash: None,
            tombstone: false,
        };

        let link = project_sync_link(Some(&existing), &envelope, None, true);

        assert_eq!(link.salesforce_id, Some("001000000000000BBB".to_string()));
        assert_eq!(link.postgres_id, Some("pg_1".to_string()));
        assert!(link.tombstone);
    }

    #[test]
    fn project_sync_link_overrides_existing_sfid_with_provided() {
        let envelope = test_envelope();
        let sfid = SalesforceId::new("001000000000000AAA").unwrap_or_else(|_| panic!("valid sfid"));
        let existing = SyncLink {
            tenant: "tenant_a".to_string(),
            object_name: "Account".to_string(),
            external_id: "ext_123".to_string(),
            salesforce_id: Some("001000000000000BBB".to_string()),
            postgres_id: Some("pg_1".to_string()),
            last_source: None,
            last_source_cursor: None,
            last_payload_hash: None,
            tombstone: false,
        };

        let link = project_sync_link(Some(&existing), &envelope, Some(&sfid), false);

        assert_eq!(link.salesforce_id, Some("001000000000000AAA".to_string()));
        assert_eq!(link.postgres_id, Some("pg_1".to_string()));
    }
}
