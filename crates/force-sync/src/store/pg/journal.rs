//! Journal write helpers for the `PostgreSQL` sync store.

use chrono::{DateTime, Utc};
use serde_json::Value;
use tokio_postgres::GenericClient;

use crate::{
    error::ForceSyncError,
    model::{ChangeEnvelope, ChangeOperation},
};

use super::PgStore;

/// Result of attempting to append a journal entry when duplicates are allowed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppendResult {
    /// The row was inserted and the returned ID is the new journal row.
    Inserted {
        /// The inserted journal row ID.
        journal_id: i64,
    },
    /// A row with the same `(source, source_cursor)` already exists.
    Duplicate,
}

struct JournalValues {
    tenant: String,
    object_name: String,
    external_id: String,
    source: &'static str,
    source_cursor: String,
    observed_at: DateTime<Utc>,
    operation: &'static str,
    tombstone: bool,
    payload: Value,
    payload_hash: [u8; 32],
}

fn journal_values(envelope: &ChangeEnvelope) -> crate::error::Result<JournalValues> {
    let cursor = envelope
        .cursor()
        .ok_or(ForceSyncError::MissingSourceCursor)?;

    Ok(JournalValues {
        tenant: envelope.sync_key().tenant().to_owned(),
        object_name: envelope.sync_key().object_name().to_owned(),
        external_id: envelope.sync_key().external_id().to_owned(),
        source: envelope.source().as_db_value(),
        source_cursor: cursor.as_db_value(),
        observed_at: envelope.observed_at(),
        operation: envelope.operation().as_db_value(),
        tombstone: matches!(envelope.operation(), ChangeOperation::Delete),
        payload: envelope.payload().clone(),
        payload_hash: envelope.payload_hash(),
    })
}

async fn insert_journal<C>(client: &C, values: &JournalValues) -> crate::error::Result<i64>
where
    C: GenericClient + Sync + ?Sized,
{
    let payload_hash = values.payload_hash.as_slice();
    let row = client
        .query_one(
            "insert into sync_journal (
                tenant,
                object_name,
                external_id,
                source,
                source_cursor,
                observed_at,
                operation,
                tombstone,
                payload,
                payload_hash,
                schema_version
            ) values (
                $1,
                $2,
                $3,
                $4,
                $5,
                $6::timestamptz,
                $7,
                $8,
                $9::jsonb,
                $10,
                1
            ) returning journal_id",
            &[
                &values.tenant,
                &values.object_name,
                &values.external_id,
                &values.source,
                &values.source_cursor,
                &values.observed_at,
                &values.operation,
                &values.tombstone,
                &values.payload,
                &payload_hash,
            ],
        )
        .await?;

    Ok(row.get(0))
}

async fn insert_journal_if_new<C>(
    client: &C,
    values: &JournalValues,
) -> Result<Option<i64>, ForceSyncError>
where
    C: GenericClient + Sync + ?Sized,
{
    let payload_hash = values.payload_hash.as_slice();
    let row = client
        .query_opt(
            "insert into sync_journal (
                tenant,
                object_name,
                external_id,
                source,
                source_cursor,
                observed_at,
                operation,
                tombstone,
                payload,
                payload_hash,
                schema_version
            ) values (
                $1,
                $2,
                $3,
                $4,
                $5,
                $6::timestamptz,
                $7,
                $8,
                $9::jsonb,
                $10,
                1
            ) on conflict (source, source_cursor) do nothing
            returning journal_id",
            &[
                &values.tenant,
                &values.object_name,
                &values.external_id,
                &values.source,
                &values.source_cursor,
                &values.observed_at,
                &values.operation,
                &values.tombstone,
                &values.payload,
                &payload_hash,
            ],
        )
        .await?;

    Ok(row.map(|row| row.get(0)))
}

impl PgStore {
    /// Appends a journal entry and returns its database identifier.
    ///
    /// # Errors
    ///
    /// Returns an error if the cursor is missing or the database write fails.
    pub async fn append_journal(&self, envelope: &ChangeEnvelope) -> crate::error::Result<i64> {
        let values = journal_values(envelope)?;
        let client = self.pool().get().await?;
        insert_journal(&**client, &values).await
    }

    /// Appends a journal entry if the `(source, source_cursor)` pair is new.
    ///
    /// # Errors
    ///
    /// Returns an error if the cursor is missing or the database write fails.
    pub async fn append_journal_if_new(
        &self,
        envelope: &ChangeEnvelope,
    ) -> crate::error::Result<AppendResult> {
        let values = journal_values(envelope)?;
        let client = self.pool().get().await?;
        insert_journal_if_new(&**client, &values)
            .await?
            .map_or_else(
                || Ok(AppendResult::Duplicate),
                |journal_id| Ok(AppendResult::Inserted { journal_id }),
            )
    }

    /// Appends a journal row in an existing transaction.
    ///
    /// This helper exists so same-transaction workflows can remain atomic
    /// without forcing the caller through another store method.
    ///
    /// # Errors
    ///
    /// Returns an error if the cursor is missing or the database write fails.
    pub async fn append_journal_in_tx<C>(
        client: &C,
        envelope: &ChangeEnvelope,
    ) -> crate::error::Result<i64>
    where
        C: GenericClient + Sync + ?Sized,
    {
        let values = journal_values(envelope)?;
        insert_journal(client, &values).await
    }

    /// Appends a journal row in an existing transaction, skipping duplicates.
    ///
    /// # Errors
    ///
    /// Returns an error if the cursor is missing or the database write fails.
    pub async fn append_journal_if_new_in_tx<C>(
        client: &C,
        envelope: &ChangeEnvelope,
    ) -> crate::error::Result<AppendResult>
    where
        C: GenericClient + Sync + ?Sized,
    {
        let values = journal_values(envelope)?;

        insert_journal_if_new(client, &values).await?.map_or_else(
            || Ok(AppendResult::Duplicate),
            |journal_id| Ok(AppendResult::Inserted { journal_id }),
        )
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use serde_json::json;

    use crate::{
        identity::SyncKey,
        model::{ChangeEnvelope, ChangeOperation, SourceCursor, SourceSystem},
    };

    use super::journal_values;

    #[test]
    fn journal_values_keep_observed_at_as_datetime() {
        let observed_at = Utc::now();
        let envelope = ChangeEnvelope::new(
            SyncKey::new("tenant", "Account", "external-1")
                .unwrap_or_else(|error| panic!("unexpected sync key construction error: {error}")),
            SourceSystem::Postgres,
            ChangeOperation::Upsert,
            observed_at,
            json!({"Name": "Acme"}),
        )
        .with_cursor(SourceCursor::PostgresLsn("lsn-1".to_owned()));

        let values = journal_values(&envelope)
            .unwrap_or_else(|error| panic!("unexpected journal values error: {error}"));

        let _: chrono::DateTime<chrono::Utc> = values.observed_at;
        let _: &serde_json::Value = &values.payload;
        assert_eq!(values.observed_at, observed_at);
        assert_eq!(values.payload, json!({"Name": "Acme"}));
    }
}
