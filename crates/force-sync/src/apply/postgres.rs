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
