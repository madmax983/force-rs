//! Postgres-first reconcile helpers for force-sync.

use futures::FutureExt;
use tokio_postgres::GenericClient;

use crate::{PgStore, error::ForceSyncError};

/// A drift candidate discovered during reconciliation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriftItem {
    /// Journal row identifier to repair.
    pub journal_id: i64,
    /// Canonical tenant identifier.
    pub tenant: String,
    /// Salesforce object name.
    pub object_name: String,
    /// Canonical external ID.
    pub external_id: String,
    /// Payload hash from the latest journal row.
    pub payload_hash: Vec<u8>,
}

async fn detect_drift_query<C>(client: &C, limit: i64) -> Result<Vec<DriftItem>, ForceSyncError>
where
    C: GenericClient + Sync + ?Sized,
{
    if limit <= 0 {
        return Ok(Vec::new());
    }

    let rows = client
        .query(
            "with latest as (
                select distinct on (tenant, object_name, external_id)
                    journal_id,
                    tenant,
                    object_name,
                    external_id,
                    payload_hash
                from sync_journal
                order by tenant, object_name, external_id, journal_id desc
            )
            select
                latest.journal_id,
                latest.tenant,
                latest.object_name,
                latest.external_id,
                latest.payload_hash
            from latest
            left join sync_link link
              on link.tenant = latest.tenant
             and link.object_name = latest.object_name
             and link.external_id = latest.external_id
            where link.link_id is null
               or link.last_payload_hash is distinct from latest.payload_hash
            order by latest.journal_id asc
            limit $1",
            &[&limit],
        )
        .await?;

    Ok(rows
        .into_iter()
        .map(|row| DriftItem {
            journal_id: row.get(0),
            tenant: row.get(1),
            object_name: row.get(2),
            external_id: row.get(3),
            payload_hash: row.get(4),
        })
        .collect())
}

async fn enqueue_repair_in_tx<C>(client: &C, journal_id: i64) -> crate::error::Result<i64>
where
    C: GenericClient + Sync + ?Sized,
{
    if let Some(task_id) = existing_repair_task_in_tx(client, journal_id).await? {
        return Ok(task_id);
    }

    PgStore::enqueue_apply_task_in_tx(client, journal_id, 0).await
}

async fn existing_repair_task_in_tx<C>(
    client: &C,
    journal_id: i64,
) -> Result<Option<i64>, ForceSyncError>
where
    C: GenericClient + Sync + ?Sized,
{
    let journal_key = journal_id.to_string();
    let row = client
        .query_opt(
            "select task_id
             from sync_task
             where task_kind = 'apply'
               and target_key = $1
               and status in ('ready', 'leased')
             order by task_id desc
             limit 1",
            &[&journal_key],
        )
        .await?;

    Ok(row.map(|row| row.get(0)))
}

/// Detects drift by comparing the latest journal row for each record to the stored link.
///
/// # Errors
///
/// Returns an error if the query fails.
pub async fn detect_drift(store: &PgStore, limit: i64) -> Result<Vec<DriftItem>, ForceSyncError> {
    let client = store.pool().get().await?;
    detect_drift_query(&**client, limit).await
}

/// Enqueues a repair task for a journal row.
///
/// # Errors
///
/// Returns an error if the task cannot be written.
pub async fn enqueue_repair(store: &PgStore, journal_id: i64) -> crate::error::Result<i64> {
    store
        .with_transaction(|tx| async move { enqueue_repair_in_tx(tx, journal_id).await }.boxed())
        .await
}

/// Runs one reconciliation pass and enqueues repair tasks for detected drift.
///
/// # Errors
///
/// Returns an error if the detection or enqueue steps fail.
pub async fn run_reconcile_once(store: &PgStore, limit: i64) -> crate::error::Result<usize> {
    store
        .with_transaction(|tx| {
            async move {
                let drift = detect_drift_query(tx, limit).await?;
                let mut enqueued = 0usize;

                for item in drift {
                    enqueue_repair_in_tx(tx, item.journal_id).await?;
                    enqueued += 1;
                }

                Ok(enqueued)
            }
            .boxed()
        })
        .await
}
