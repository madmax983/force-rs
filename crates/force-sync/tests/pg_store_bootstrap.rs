//! Integration test for the `PostgreSQL` store bootstrap path.

mod support;

use futures::FutureExt;

#[tokio::test]
#[ignore = "requires FORCE_SYNC_TEST_DATABASE_URL"]
async fn pg_store_can_open_transaction() -> Result<(), force_sync::error::ForceSyncError> {
    let pool = support::postgres::test_pool();
    support::postgres::reset_schema(&pool).await?;
    force_sync::store::pg::migrate(&pool).await?;

    let store = force_sync::store::pg::PgStore::new(pool.clone());

    let value: i64 = store
        .with_client(|client| async move {
            client
                .query_one("select 1", &[])
                .await
                .map(|row| row.get(0))
        })
        .await?;
    assert_eq!(value, 1);

    let transaction_value: i64 = store
        .with_transaction(|client| {
            async move { Ok(client.query_one("select 1", &[]).await?.get(0)) }.boxed()
        })
        .await?;
    assert_eq!(transaction_value, 1);

    Ok(())
}
