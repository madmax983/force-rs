//! Embedded Postgres migration runner for force-sync.

use deadpool_postgres::Pool;

use crate::error::ForceSyncError;

const MIGRATIONS: &[(i32, &str)] = &[
    (1, include_str!("../../../migrations/0001_init.sql")),
    (
        2,
        include_str!("../../../migrations/0002_force_sync_outbox.sql"),
    ),
];

/// Applies the force-sync schema migrations in version order.
///
/// # Errors
///
/// Returns a database or pool error if the migration cannot be applied.
pub async fn migrate(pool: &Pool) -> crate::error::Result<()> {
    let mut client = pool.get().await?;
    let transaction = client.transaction().await?;

    let mut migration_table_exists: bool = transaction
        .query_one(
            "select to_regclass('public.force_sync_schema_migrations') is not null",
            &[],
        )
        .await?
        .get(0);

    for &(version, sql) in MIGRATIONS {
        let already_applied = if migration_table_exists {
            transaction
                .query_opt(
                    "select 1 from force_sync_schema_migrations where version = $1",
                    &[&version],
                )
                .await?
                .is_some()
        } else {
            false
        };

        if already_applied {
            continue;
        }

        transaction.batch_execute(sql).await?;
        migration_table_exists = true;
    }

    transaction.commit().await?;
    Ok(())
}
