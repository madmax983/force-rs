//! Integration test for the initial force-sync `Postgres` schema.

mod support;

#[tokio::test]
#[ignore = "requires FORCE_SYNC_TEST_DATABASE_URL"]
async fn applies_initial_schema() -> force_sync::Result<()> {
    let pool = support::postgres::test_pool();
    support::postgres::reset_schema(&pool).await?;

    force_sync::migrate(&pool).await?;

    let client = pool.get().await?;
    let rows = client
        .query("select to_regclass('public.sync_journal')::text", &[])
        .await?;

    assert_eq!(
        rows[0].get::<_, Option<String>>(0).as_deref(),
        Some("sync_journal")
    );
    Ok(())
}
