//! Conflict repository helpers for the `PostgreSQL` sync store.

use serde_json::Value;
use tokio_postgres::GenericClient;

use crate::error::Result;

use super::PgStore;

/// Conflict record stored for later review.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Debug, Clone, PartialEq)]
pub struct SyncConflict {
    /// Tenant identifier.
    pub tenant: String,
    /// Salesforce object name.
    pub object_name: String,
    /// Canonical external ID.
    pub external_id: String,
    /// Field that conflicted.
    pub field_name: String,
    /// Left-hand value.
    pub left_value: Value,
    /// Right-hand value.
    pub right_value: Value,
    /// Conflict resolution policy or outcome.
    pub resolution: Option<String>,
}

async fn insert_conflict_query<C>(client: &C, conflict: &SyncConflict) -> Result<i64>
where
    C: GenericClient + Sync + ?Sized,
{
    let row = client
        .query_one(
            "insert into sync_conflict (
                tenant,
                object_name,
                external_id,
                field_name,
                left_value,
                right_value,
                resolution
            ) values ($1, $2, $3, $4, $5::jsonb, $6::jsonb, $7)
            returning conflict_id",
            &[
                &conflict.tenant,
                &conflict.object_name,
                &conflict.external_id,
                &conflict.field_name,
                &conflict.left_value,
                &conflict.right_value,
                &conflict.resolution,
            ],
        )
        .await?;

    Ok(row.get(0))
}

impl PgStore {
    /// Inserts a conflict row and returns its database identifier.
    ///
    /// # Errors
    ///
    /// Returns an error if the database write fails.
    pub async fn insert_conflict(&self, conflict: &SyncConflict) -> Result<i64> {
        let client = self.pool().get().await?;
        insert_conflict_query(&**client, conflict).await
    }
}
