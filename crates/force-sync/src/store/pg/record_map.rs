//! Salesforce record ID map for sparse CDC events.
//!
//! A Salesforce update or delete change event names its changed fields only.
//! It usually carries no external ID, so capture resolves the record key from
//! the record ID in the event header.

use tokio_postgres::GenericClient;

use serde_json::Value;

use crate::error::ForceSyncError;

use super::PgStore;

impl PgStore {
    /// Stores the merged state of a record after an applied change.
    ///
    /// # Errors
    ///
    /// Returns an error if the database write fails.
    pub(crate) async fn put_record_state(
        &self,
        tenant: &str,
        object_name: &str,
        external_id: &str,
        payload: &Value,
    ) -> Result<(), ForceSyncError> {
        let payload = payload.to_string();
        self.pool()
            .get()
            .await?
            .execute(
                "insert into sync_record_state (tenant, object_name, external_id, payload)
                 values ($1, $2, $3, $4::text::jsonb)
                 on conflict (tenant, object_name, external_id) do update
                     set payload = excluded.payload,
                         updated_at = now()",
                &[&tenant, &object_name, &external_id, &payload],
            )
            .await?;
        Ok(())
    }
}

/// Remembers which external ID keys a Salesforce record.
///
/// # Errors
///
/// Returns an error if the database write fails.
pub(crate) async fn remember_salesforce_record_in_tx<C>(
    client: &C,
    tenant: &str,
    object_name: &str,
    salesforce_id: &str,
    external_id: &str,
) -> Result<(), ForceSyncError>
where
    C: GenericClient + Sync + ?Sized,
{
    client
        .execute(
            "insert into sync_salesforce_record (tenant, object_name, salesforce_id, external_id)
             values ($1, $2, $3, $4)
             on conflict (tenant, object_name, salesforce_id) do update
                 set external_id = excluded.external_id,
                     updated_at = now()
                 where sync_salesforce_record.external_id is distinct from excluded.external_id",
            &[&tenant, &object_name, &salesforce_id, &external_id],
        )
        .await?;
    Ok(())
}

/// Finds the external ID of a Salesforce record.
///
/// A record that an earlier change event named is in the record map. A record
/// created from Postgres is known only by its sync link. The map wins when
/// both know the record.
///
/// # Errors
///
/// Returns an error if the database query fails.
pub(crate) async fn external_id_for_salesforce_record_in_tx<C>(
    client: &C,
    tenant: &str,
    object_name: &str,
    salesforce_id: &str,
) -> Result<Option<String>, ForceSyncError>
where
    C: GenericClient + Sync + ?Sized,
{
    let row = client
        .query_opt(
            "select external_id from (
                 select external_id, 0 as source_rank, now() as updated_at
                 from sync_salesforce_record
                 where tenant = $1 and object_name = $2 and salesforce_id = $3
                 union all
                 select external_id, 1 as source_rank, updated_at
                 from sync_link
                 where tenant = $1 and object_name = $2 and salesforce_id = $3
             ) as known
             order by source_rank, updated_at desc
             limit 1",
            &[&tenant, &object_name, &salesforce_id],
        )
        .await?;
    Ok(row.map(|row| row.get(0)))
}
