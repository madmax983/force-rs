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
        model::{ChangeOperation, SourceCursor, SourceSystem},
    };
    use chrono::Utc;
    use serde_json::json;

    #[test]
    fn should_project_sync_link_from_envelope_without_existing_link() {
        let envelope = ChangeEnvelope::new(
            SyncKey::new("tenant1", "Account", "ext-1")
                .unwrap_or_else(|_| panic!("Failed to unwrap")),
            SourceSystem::Salesforce,
            ChangeOperation::Upsert,
            Utc::now(),
            json!({"Name": "Test"}),
        )
        .with_cursor(SourceCursor::SalesforceReplayId(123));
        let sf_id =
            SalesforceId::new("001000000000000AAA").unwrap_or_else(|_| panic!("Failed to unwrap"));

        let link = project_sync_link(None, &envelope, Some(&sf_id), false);

        assert_eq!(link.tenant, "tenant1");
        assert_eq!(link.object_name, "Account");
        assert_eq!(link.external_id, "ext-1");
        assert_eq!(link.salesforce_id.as_deref(), Some("001000000000000AAA"));
        assert_eq!(link.postgres_id, None);
        assert_eq!(link.last_source.as_deref(), Some("salesforce"));
        assert_eq!(
            link.last_source_cursor.as_deref(),
            Some("salesforce-replay-id:123")
        );
        assert_eq!(
            link.last_payload_hash.as_deref(),
            Some(envelope.payload_hash().as_slice())
        );
        assert!(!link.tombstone);
    }

    #[test]
    fn should_project_sync_link_preferring_provided_salesforce_id_over_existing() {
        let envelope = ChangeEnvelope::new(
            SyncKey::new("tenant1", "Account", "ext-1")
                .unwrap_or_else(|_| panic!("Failed to unwrap")),
            SourceSystem::Postgres,
            ChangeOperation::Upsert,
            Utc::now(),
            json!({"Name": "Test"}),
        );
        let existing_link = SyncLink {
            tenant: "tenant1".to_string(),
            object_name: "Account".to_string(),
            external_id: "ext-1".to_string(),
            salesforce_id: Some("001000000000000OLD".to_string()),
            postgres_id: Some("pg-1".to_string()),
            last_source: None,
            last_source_cursor: None,
            last_payload_hash: None,
            tombstone: false,
        };
        // Use a valid Salesforce ID to avoid InvalidChecksum error
        let new_sf_id =
            SalesforceId::new("001000000000000AAA").unwrap_or_else(|_| panic!("Failed to unwrap"));

        let link = project_sync_link(Some(&existing_link), &envelope, Some(&new_sf_id), true);

        assert_eq!(link.salesforce_id.as_deref(), Some("001000000000000AAA"));
        assert_eq!(link.postgres_id.as_deref(), Some("pg-1"));
        assert_eq!(link.last_source.as_deref(), Some("postgres"));
        assert_eq!(link.last_source_cursor, None);
        assert!(link.tombstone);
    }

    #[test]
    fn should_project_sync_link_falling_back_to_existing_salesforce_id_when_none_provided() {
        let envelope = ChangeEnvelope::new(
            SyncKey::new("tenant1", "Account", "ext-1")
                .unwrap_or_else(|_| panic!("Failed to unwrap")),
            SourceSystem::Salesforce,
            ChangeOperation::Upsert,
            Utc::now(),
            json!({"Name": "Test"}),
        );
        let existing_link = SyncLink {
            tenant: "tenant1".to_string(),
            object_name: "Account".to_string(),
            external_id: "ext-1".to_string(),
            salesforce_id: Some("001000000000000OLD".to_string()),
            postgres_id: Some("pg-1".to_string()),
            last_source: None,
            last_source_cursor: None,
            last_payload_hash: None,
            tombstone: false,
        };

        let link = project_sync_link(Some(&existing_link), &envelope, None, false);

        assert_eq!(link.salesforce_id.as_deref(), Some("001000000000000OLD"));
        assert_eq!(link.postgres_id.as_deref(), Some("pg-1"));
    }
}
