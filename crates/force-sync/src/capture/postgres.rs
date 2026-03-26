//! `PostgreSQL` outbox capture for force-sync.

use futures::FutureExt;
use serde_json::Value;
use tokio_postgres::GenericClient;

use crate::{
    error::ForceSyncError,
    identity::SyncKey,
    model::{ChangeEnvelope, ChangeOperation, SourceCursor, SourceSystem},
    store::pg::{AppendResult, DeadLetter, PgStore},
};

struct OutboxRow {
    outbox_id: i64,
    tenant: String,
    object_name: String,
    external_id: String,
    source_cursor: String,
    op: String,
    tombstone: bool,
    payload_text: String,
    created_at: chrono::DateTime<chrono::Utc>,
}

fn outbox_operation(op: &str, tombstone: bool) -> Result<ChangeOperation, ForceSyncError> {
    match (op, tombstone) {
        ("upsert", false) => Ok(ChangeOperation::Upsert),
        ("delete", true) => Ok(ChangeOperation::Delete),
        ("upsert", true) | ("delete", false) => Err(ForceSyncError::InvalidOutboxOperation {
            op: format!("{op}:{tombstone}"),
        }),
        (other_op, _) => Err(ForceSyncError::InvalidOutboxOperation {
            op: other_op.to_string(),
        }),
    }
}

fn outbox_source_cursor(raw: &str) -> Result<SourceCursor, ForceSyncError> {
    if raw.starts_with("postgres-lsn:")
        || raw.starts_with("salesforce-replay-id:")
        || raw.starts_with("snapshot:")
    {
        return Err(ForceSyncError::InvalidOutboxCursor {
            cursor: raw.to_string(),
        });
    }

    Ok(SourceCursor::PostgresLsn(raw.to_string()))
}

fn outbox_envelope(row: &OutboxRow) -> Result<ChangeEnvelope, ForceSyncError> {
    let payload: Value = serde_json::from_str(&row.payload_text)?;
    let sync_key = SyncKey::new(
        row.tenant.clone(),
        row.object_name.clone(),
        row.external_id.clone(),
    )?;
    let operation = outbox_operation(&row.op, row.tombstone)?;

    Ok(ChangeEnvelope::new(
        sync_key,
        SourceSystem::Postgres,
        operation,
        row.created_at,
        payload,
    )
    .with_cursor(outbox_source_cursor(&row.source_cursor)?))
}

const fn row_content_error(error: &ForceSyncError) -> bool {
    matches!(
        error,
        ForceSyncError::Json(_)
            | ForceSyncError::InvalidOutboxOperation { .. }
            | ForceSyncError::InvalidOutboxCursor { .. }
            | ForceSyncError::EmptySyncKeyPart { .. }
    )
}

async fn quarantine_row<C>(
    client: &C,
    row: &OutboxRow,
    error: &ForceSyncError,
) -> Result<(), ForceSyncError>
where
    C: GenericClient + Sync + ?Sized,
{
    let payload = serde_json::from_str::<Value>(&row.payload_text).ok();
    let dead_letter = DeadLetter {
        task_id: None,
        tenant: Some(row.tenant.clone()),
        object_name: Some(row.object_name.clone()),
        external_id: Some(row.external_id.clone()),
        error_message: error.to_string(),
        payload,
    };

    crate::store::pg::dead_letter::insert_dead_letter_in_tx(client, &dead_letter).await?;
    client
        .execute(
            "update force_sync_outbox
             set processed_at = now()
             where outbox_id = $1
               and processed_at is null",
            &[&row.outbox_id],
        )
        .await?;
    Ok(())
}

async fn capture_batch_in_tx<C>(
    client: &C,
    limit: i64,
    priority: i32,
) -> Result<usize, ForceSyncError>
where
    C: GenericClient + Sync + ?Sized,
{
    if limit <= 0 {
        return Ok(0);
    }

    let rows = client
        .query(
            "select outbox_id, tenant, object_name, external_id, source_cursor, op, tombstone, payload::text as payload_text, created_at
             from force_sync_outbox
             where processed_at is null
             order by created_at asc, outbox_id asc
             for update skip locked
             limit $1",
            &[&limit],
        )
        .await?;

    let mut processed = 0usize;

    for row in rows {
        let outbox_row = OutboxRow {
            outbox_id: row.get("outbox_id"),
            tenant: row.get("tenant"),
            object_name: row.get("object_name"),
            external_id: row.get("external_id"),
            source_cursor: row.get("source_cursor"),
            op: row.get("op"),
            tombstone: row.get("tombstone"),
            payload_text: row.get("payload_text"),
            created_at: row.get("created_at"),
        };

        match outbox_envelope(&outbox_row) {
            Ok(envelope) => match PgStore::append_journal_if_new_in_tx(client, &envelope).await? {
                AppendResult::Inserted { journal_id } => {
                    PgStore::enqueue_apply_task_in_tx(client, journal_id, priority).await?;
                    client
                        .execute(
                            "update force_sync_outbox
                             set processed_at = now()
                             where outbox_id = $1
                               and processed_at is null",
                            &[&outbox_row.outbox_id],
                        )
                        .await?;
                    processed += 1;
                }
                AppendResult::Duplicate => {
                    client
                        .execute(
                            "update force_sync_outbox
                             set processed_at = now()
                             where outbox_id = $1
                               and processed_at is null",
                            &[&outbox_row.outbox_id],
                        )
                        .await?;
                    processed += 1;
                }
            },
            Err(error) if row_content_error(&error) => {
                quarantine_row(client, &outbox_row, &error).await?;
                processed += 1;
            }
            Err(error) => return Err(error),
        }
    }

    Ok(processed)
}

/// Captures a batch of unprocessed `PostgreSQL` outbox rows into the sync journal.
///
/// # Errors
///
/// Returns a database error if the transaction cannot be opened or a row
/// cannot be converted into a sync envelope.
pub async fn capture_batch(
    store: &PgStore,
    limit: i64,
    priority: i32,
) -> Result<usize, ForceSyncError> {
    store
        .with_transaction(|tx| {
            async move { capture_batch_in_tx(tx, limit, priority).await }.boxed()
        })
        .await
}
