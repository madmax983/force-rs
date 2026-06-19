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
    use crate::{
        identity::SyncKey,
        model::{ChangeEnvelope, ChangeOperation, SourceCursor, SourceSystem},
    };
    use chrono::Utc;
    use force::types::SalesforceId;
    use serde_json::json;

    #[test]
    fn project_sync_link_without_existing_or_cursor() {
        let sync_key =
            SyncKey::new("tenant", "Account", "abc").unwrap_or_else(|_| panic!("valid key"));
        let envelope = ChangeEnvelope::new(
            sync_key,
            SourceSystem::Salesforce,
            ChangeOperation::Upsert,
            Utc::now(),
            json!({"Name": "Acme"}),
        );
        let id = SalesforceId::new("001000000000001AAA").unwrap_or_else(|_| panic!("valid id"));

        let link = project_sync_link(None, &envelope, Some(&id), false);

        assert_eq!(link.tenant, "tenant");
        assert_eq!(link.object_name, "Account");
        assert_eq!(link.external_id, "abc");
        assert_eq!(link.salesforce_id, Some("001000000000001AAA".to_owned()));
        assert_eq!(link.postgres_id, None);
        assert_eq!(link.last_source, Some("salesforce".to_owned()));
        assert_eq!(link.last_source_cursor, None);
        assert_eq!(
            link.last_payload_hash,
            Some(envelope.payload_hash().to_vec())
        );
        assert!(!link.tombstone);
    }

    #[test]
    fn project_sync_link_with_existing_and_cursor() {
        let sync_key =
            SyncKey::new("tenant", "Account", "abc").unwrap_or_else(|_| panic!("valid key"));
        let envelope = ChangeEnvelope::new(
            sync_key,
            SourceSystem::Postgres,
            ChangeOperation::Upsert,
            Utc::now(),
            json!({"Name": "Acme"}),
        )
        .with_cursor(SourceCursor::PostgresLsn("0/1234".to_owned()));

        let existing = SyncLink {
            tenant: "tenant".to_owned(),
            object_name: "Account".to_owned(),
            external_id: "abc".to_owned(),
            salesforce_id: Some("001000000000001AAA".to_owned()),
            postgres_id: Some("uuid-1234".to_owned()),
            last_source: Some("salesforce".to_owned()),
            last_source_cursor: None,
            last_payload_hash: None,
            tombstone: false,
        };

        let link = project_sync_link(Some(&existing), &envelope, None, true);

        assert_eq!(link.tenant, "tenant");
        assert_eq!(link.object_name, "Account");
        assert_eq!(link.external_id, "abc");
        assert_eq!(link.salesforce_id, Some("001000000000001AAA".to_owned())); // Fallback to existing
        assert_eq!(link.postgres_id, Some("uuid-1234".to_owned())); // Carried over
        assert_eq!(link.last_source, Some("postgres".to_owned())); // Updated
        assert_eq!(
            link.last_source_cursor,
            Some("postgres-lsn:0/1234".to_owned())
        ); // Updated
        assert_eq!(
            link.last_payload_hash,
            Some(envelope.payload_hash().to_vec())
        );
        assert!(link.tombstone);
    }
}
