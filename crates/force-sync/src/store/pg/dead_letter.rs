//! Dead-letter repository helpers for the `PostgreSQL` sync store.

use crate::error::Result;
use serde_json::Value;
use tokio_postgres::GenericClient;

use super::PgStore;

/// Dead-letter row captured for operator review.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Debug, Clone, PartialEq)]
pub struct DeadLetter {
    /// Optional task identifier.
    pub task_id: Option<i64>,
    /// Optional tenant identifier.
    pub tenant: Option<String>,
    /// Optional Salesforce object name.
    pub object_name: Option<String>,
    /// Optional canonical external ID.
    pub external_id: Option<String>,
    /// Terminal error message.
    pub error_message: String,
    /// Optional payload captured at failure time.
    pub payload: Option<Value>,
}

async fn insert_dead_letter_query<C>(client: &C, dead_letter: &DeadLetter) -> Result<i64>
where
    C: GenericClient + Sync + ?Sized,
{
    let row = client
        .query_one(
            "insert into sync_dead_letter (
                task_id,
                tenant,
                object_name,
                external_id,
                error_message,
                payload
            ) values ($1, $2, $3, $4, $5, $6::jsonb)
            returning dead_letter_id",
            &[
                &dead_letter.task_id,
                &dead_letter.tenant,
                &dead_letter.object_name,
                &dead_letter.external_id,
                &dead_letter.error_message,
                &dead_letter.payload,
            ],
        )
        .await?;

    Ok(row.get(0))
}

/// Inserts a dead-letter row inside an existing transaction or client session.
///
/// # Errors
///
/// Returns an error if the database write fails.
pub async fn insert_dead_letter_in_tx<C>(client: &C, dead_letter: &DeadLetter) -> Result<i64>
where
    C: GenericClient + Sync + ?Sized,
{
    insert_dead_letter_query(client, dead_letter).await
}

impl PgStore {
    /// Inserts a dead-letter row and returns its database identifier.
    ///
    /// # Errors
    ///
    /// Returns an error if the database write fails.
    pub async fn insert_dead_letter(&self, dead_letter: &DeadLetter) -> Result<i64> {
        let client = self.pool().get().await?;
        insert_dead_letter_query(&**client, dead_letter).await
    }
}
