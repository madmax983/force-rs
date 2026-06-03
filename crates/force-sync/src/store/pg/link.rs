//! Link repository helpers for the `PostgreSQL` sync store.

use crate::error::Result;
use tokio_postgres::GenericClient;

use crate::error::ForceSyncError;

use super::PgStore;

/// Canonical sync link row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncLink {
    /// Tenant identifier.
    pub tenant: String,
    /// Salesforce object name.
    pub object_name: String,
    /// Canonical external ID.
    pub external_id: String,
    /// Salesforce ID alias, if known.
    pub salesforce_id: Option<String>,
    /// Postgres ID alias, if known.
    pub postgres_id: Option<String>,
    /// Source that last wrote the link.
    pub last_source: Option<String>,
    /// Source cursor for the last write.
    pub last_source_cursor: Option<String>,
    /// Payload hash for the last write.
    pub last_payload_hash: Option<Vec<u8>>,
    /// Whether the link represents a tombstone.
    pub tombstone: bool,
}

fn link_from_row(row: &tokio_postgres::Row) -> SyncLink {
    SyncLink {
        tenant: row.get(0),
        object_name: row.get(1),
        external_id: row.get(2),
        salesforce_id: row.get(3),
        postgres_id: row.get(4),
        last_source: row.get(5),
        last_source_cursor: row.get(6),
        last_payload_hash: row.get(7),
        tombstone: row.get(8),
    }
}

async fn put_link_query<C>(client: &C, link: &SyncLink) -> Result<i64>
where
    C: GenericClient + Sync + ?Sized,
{
    let payload_hash = link.last_payload_hash.as_deref();
    let row = client
        .query_one(
            "insert into sync_link (
                tenant,
                object_name,
                external_id,
                salesforce_id,
                postgres_id,
                last_source,
                last_source_cursor,
                last_payload_hash,
                tombstone,
                updated_at
            ) values (
                $1,
                $2,
                $3,
                $4,
                $5,
                $6,
                $7,
                $8,
                $9,
                now()
            ) on conflict (tenant, object_name, external_id) do update set
                salesforce_id = excluded.salesforce_id,
                postgres_id = excluded.postgres_id,
                last_source = excluded.last_source,
                last_source_cursor = excluded.last_source_cursor,
                last_payload_hash = excluded.last_payload_hash,
                tombstone = excluded.tombstone,
                updated_at = now()
            returning link_id",
            &[
                &link.tenant,
                &link.object_name,
                &link.external_id,
                &link.salesforce_id,
                &link.postgres_id,
                &link.last_source,
                &link.last_source_cursor,
                &payload_hash,
                &link.tombstone,
            ],
        )
        .await?;

    Ok(row.get(0))
}

async fn get_link_query<C>(
    client: &C,
    tenant: &str,
    object_name: &str,
    external_id: &str,
) -> Result<Option<SyncLink>, ForceSyncError>
where
    C: GenericClient + Sync + ?Sized,
{
    let row = client
        .query_opt(
            "select tenant, object_name, external_id, salesforce_id, postgres_id,
                    last_source, last_source_cursor, last_payload_hash, tombstone
             from sync_link
             where tenant = $1 and object_name = $2 and external_id = $3",
            &[&tenant, &object_name, &external_id],
        )
        .await?;

    Ok(row.map(|row| link_from_row(&row)))
}

impl PgStore {
    /// Upserts a link row and returns the database identifier.
    ///
    /// # Errors
    ///
    /// Returns an error if the database write fails.
    pub async fn put_link(&self, link: &SyncLink) -> Result<i64> {
        let client = self.pool().get().await?;
        put_link_query(&**client, link).await
    }

    /// Loads a link row by canonical identity.
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    pub async fn get_link(
        &self,
        tenant: &str,
        object_name: &str,
        external_id: &str,
    ) -> Result<Option<SyncLink>, ForceSyncError> {
        let client = self.pool().get().await?;
        get_link_query(&**client, tenant, object_name, external_id).await
    }
}
