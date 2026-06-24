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
    use force::types::SalesforceId;
    use serde_json::json;

    fn envelope() -> ChangeEnvelope {
        ChangeEnvelope::new(
            SyncKey::new("tenant", "Account", "external-1")
                .unwrap_or_else(|e| panic!("failed to create key: {e}")),
            SourceSystem::Postgres,
            ChangeOperation::Upsert,
            chrono::Utc::now(),
            json!({"Name": "Acme"}),
        )
        .with_cursor(SourceCursor::PostgresLsn("lsn123".to_owned()))
    }

    #[test]
    fn should_project_new_sync_link_without_existing() {
        let env = envelope();
        let sf_id = SalesforceId::new("001000000000001AAA").unwrap_or_else(|e| panic!("invalid id: {e}"));
        let link = project_sync_link(None, &env, Some(&sf_id), false);

        assert_eq!(link.tenant, "tenant");
        assert_eq!(link.object_name, "Account");
        assert_eq!(link.external_id, "external-1");
        assert_eq!(link.salesforce_id.as_deref(), Some("001000000000001AAA"));
        assert_eq!(link.postgres_id, None);
        assert_eq!(link.last_source.as_deref(), Some("postgres"));
        assert_eq!(link.last_source_cursor.as_deref(), Some("postgres-lsn:lsn123"));
        assert_eq!(link.last_payload_hash, Some(env.payload_hash().to_vec()));
        assert!(!link.tombstone);
    }

    #[test]
    fn should_project_with_existing_preserving_postgres_id() {
        let env = envelope();
        let sf_id = SalesforceId::new("001000000000001AAA").unwrap_or_else(|e| panic!("invalid id: {e}"));
        let existing = SyncLink {
            tenant: "tenant".to_owned(),
            object_name: "Account".to_owned(),
            external_id: "external-1".to_owned(),
            salesforce_id: None,
            postgres_id: Some("pg-uuid".to_owned()),
            last_source: None,
            last_source_cursor: None,
            last_payload_hash: None,
            tombstone: false,
        };

        let link = project_sync_link(Some(&existing), &env, Some(&sf_id), true);

        assert_eq!(link.postgres_id.as_deref(), Some("pg-uuid"));
        assert_eq!(link.salesforce_id.as_deref(), Some("001000000000001AAA"));
        assert!(link.tombstone);
    }

    #[test]
    fn should_fallback_to_existing_salesforce_id_if_none_provided() {
        let env = envelope();
        let existing = SyncLink {
            tenant: "tenant".to_owned(),
            object_name: "Account".to_owned(),
            external_id: "external-1".to_owned(),
            salesforce_id: Some("001000000000002BBB".to_owned()),
            postgres_id: None,
            last_source: None,
            last_source_cursor: None,
            last_payload_hash: None,
            tombstone: false,
        };

        let link = project_sync_link(Some(&existing), &env, None, false);

        assert_eq!(link.salesforce_id.as_deref(), Some("001000000000002BBB"));
    }
}
