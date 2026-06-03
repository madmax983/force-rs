//! Checkpoint repository helpers for the `PostgreSQL` sync store.

use crate::error::Result;
use tokio_postgres::GenericClient;

use crate::error::ForceSyncError;

use super::PgStore;

/// Stored checkpoint state for a capture stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckpointState {
    /// Stream name that owns the checkpoint.
    pub stream_name: String,
    /// Monotonic numeric cursor position.
    pub cursor_position: i64,
    /// Original cursor string.
    pub cursor: Option<String>,
}

async fn advance_checkpoint_if_greater_query<C>(
    client: &C,
    stream_name: &str,
    cursor_position: i64,
    cursor: &str,
) -> Result<u64>
where
    C: GenericClient + Sync + ?Sized,
{
    let rows = client
        .execute(
            "insert into sync_checkpoint (stream_name, cursor_position, cursor)
             values ($1, $2, $3)
             on conflict (stream_name) do update set
                 cursor_position = excluded.cursor_position,
                 cursor = excluded.cursor,
                 updated_at = now()
             where sync_checkpoint.cursor_position < excluded.cursor_position",
            &[&stream_name, &cursor_position, &cursor],
        )
        .await?;

    Ok(rows)
}

async fn get_checkpoint_query<C>(
    client: &C,
    stream_name: &str,
) -> Result<Option<CheckpointState>, ForceSyncError>
where
    C: GenericClient + Sync + ?Sized,
{
    let row = client
        .query_opt(
            "select stream_name, cursor_position, cursor
             from sync_checkpoint
             where stream_name = $1",
            &[&stream_name],
        )
        .await?;

    Ok(row.map(|row| CheckpointState {
        stream_name: row.get(0),
        cursor_position: row.get(1),
        cursor: row.get(2),
    }))
}

impl PgStore {
    /// Advances a checkpoint only if the new position is greater than the stored one.
    ///
    /// Returns the number of affected rows.
    ///
    /// # Errors
    ///
    /// Returns an error if the database write fails.
    pub async fn advance_checkpoint_if_greater(
        &self,
        stream_name: impl AsRef<str>,
        cursor_position: i64,
        cursor: impl AsRef<str>,
    ) -> Result<u64> {
        let stream_name = stream_name.as_ref().to_owned();
        let cursor = cursor.as_ref().to_owned();
        let client = self.pool().get().await?;
        advance_checkpoint_if_greater_query(&**client, &stream_name, cursor_position, &cursor).await
    }

    /// Advances a checkpoint in an existing transaction when the position increases.
    ///
    /// # Errors
    ///
    /// Returns an error if the database write fails.
    pub async fn advance_checkpoint_if_greater_in_tx<C>(
        client: &C,
        stream_name: &str,
        cursor_position: i64,
        cursor: &str,
    ) -> Result<u64>
    where
        C: GenericClient + Sync + ?Sized,
    {
        advance_checkpoint_if_greater_query(client, stream_name, cursor_position, cursor).await
    }

    /// Loads the stored checkpoint for a stream, if one exists.
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    pub async fn get_checkpoint(
        &self,
        stream_name: impl AsRef<str>,
    ) -> Result<Option<CheckpointState>, ForceSyncError> {
        let client = self.pool().get().await?;
        get_checkpoint_query(&**client, stream_name.as_ref()).await
    }
}
