//! Integration tests for `PostgreSQL` outbox capture.

mod support;

use serde_json::json;

use force_sync::{capture_batch, ForceSyncError, PgStore};

struct OutboxSeed<'a> {
    tenant: &'a str,
    object_name: &'a str,
    external_id: &'a str,
    source_cursor: &'a str,
    op: &'a str,
    tombstone: bool,
    payload: &'a serde_json::Value,
}

async fn insert_outbox_row(
    pool: &deadpool_postgres::Pool,
    seed: &OutboxSeed<'_>,
) -> Result<i64, ForceSyncError> {
    let client = pool.get().await?;
    let row = client
        .query_one(
            "insert into force_sync_outbox (
                tenant,
                object_name,
                external_id,
                source_cursor,
                op,
                tombstone,
                payload
            ) values (
                $1,
                $2,
                $3,
                $4,
                $5,
                $6,
                $7::jsonb
            ) returning outbox_id",
            &[
                &seed.tenant,
                &seed.object_name,
                &seed.external_id,
                &seed.source_cursor,
                &seed.op,
                &seed.tombstone,
                &seed.payload,
            ],
        )
        .await?;

    Ok(row.get(0))
}

async fn dead_letter_count(pool: &deadpool_postgres::Pool) -> Result<i64, ForceSyncError> {
    let client = pool.get().await?;
    let row = client
        .query_one("select count(*) from sync_dead_letter", &[])
        .await?;
    Ok(row.get(0))
}

async fn error_message_for_external_id(
    pool: &deadpool_postgres::Pool,
    external_id: &str,
) -> Result<Option<String>, ForceSyncError> {
    let client = pool.get().await?;
    let row = client
        .query_opt(
            "select error_message from sync_dead_letter where external_id = $1 order by dead_letter_id desc limit 1",
            &[&external_id],
        )
        .await?;

    Ok(row.map(|row| row.get(0)))
}

#[tokio::test]
#[ignore = "requires FORCE_SYNC_TEST_DATABASE_URL"]
async fn outbox_rows_are_captured_into_the_journal_and_marked_processed()
-> Result<(), ForceSyncError> {
    let pool = support::postgres::test_pool();
    support::postgres::reset_schema(&pool).await?;
    force_sync::migrate(&pool).await?;

    let payload = json!({"Name": "Acme"});
    let seed = OutboxSeed {
        tenant: "tenant",
        object_name: "Account",
        external_id: "external-1",
        source_cursor: "postgres-lsn-1",
        op: "upsert",
        tombstone: false,
        payload: &payload,
    };
    let outbox_id = insert_outbox_row(&pool, &seed).await?;

    let store = PgStore::new(pool.clone());
    let processed = capture_batch(&store, 10, 25).await?;
    assert_eq!(processed, 1);

    let client = pool.get().await?;
    let journal = client
        .query_one(
            "select tenant, object_name, external_id, source, source_cursor, operation
             from sync_journal
             where external_id = $1",
            &[&"external-1"],
        )
        .await?;
    assert_eq!(journal.get::<_, String>(0), "tenant");
    assert_eq!(journal.get::<_, String>(1), "Account");
    assert_eq!(journal.get::<_, String>(2), "external-1");
    assert_eq!(journal.get::<_, String>(3), "postgres");
    assert_eq!(journal.get::<_, String>(4), "postgres-lsn:postgres-lsn-1");
    assert_eq!(journal.get::<_, String>(5), "upsert");

    let task_count = client
        .query_one(
            "select count(*) from sync_task where task_kind = 'apply'",
            &[],
        )
        .await?;
    assert_eq!(task_count.get::<_, i64>(0), 1);

    let processed_row = client
        .query_one(
            "select processed_at is not null from force_sync_outbox where outbox_id = $1",
            &[&outbox_id],
        )
        .await?;
    assert!(processed_row.get::<_, bool>(0));
    Ok(())
}

#[tokio::test]
#[ignore = "requires FORCE_SYNC_TEST_DATABASE_URL"]
async fn duplicate_source_cursors_do_not_enqueue_duplicate_tasks() -> Result<(), ForceSyncError> {
    let pool = support::postgres::test_pool();
    support::postgres::reset_schema(&pool).await?;
    force_sync::migrate(&pool).await?;

    let payload = json!({"Name": "Acme"});
    let first_outbox_id = insert_outbox_row(
        &pool,
        &OutboxSeed {
            tenant: "tenant",
            object_name: "Account",
            external_id: "external-1",
            source_cursor: "postgres-lsn-dup",
            op: "upsert",
            tombstone: false,
            payload: &payload,
        },
    )
    .await?;
    let second_outbox_id = insert_outbox_row(
        &pool,
        &OutboxSeed {
            tenant: "tenant",
            object_name: "Account",
            external_id: "external-2",
            source_cursor: "postgres-lsn-dup",
            op: "upsert",
            tombstone: false,
            payload: &payload,
        },
    )
    .await?;

    let store = PgStore::new(pool.clone());
    let processed = capture_batch(&store, 10, 25).await?;
    assert_eq!(processed, 2);

    let client = pool.get().await?;
    let journal_count = client
        .query_one(
            "select count(*) from sync_journal where source_cursor = $1",
            &[&"postgres-lsn:postgres-lsn-dup"],
        )
        .await?;
    assert_eq!(journal_count.get::<_, i64>(0), 1);

    let task_count = client
        .query_one(
            "select count(*) from sync_task where task_kind = 'apply'",
            &[],
        )
        .await?;
    assert_eq!(task_count.get::<_, i64>(0), 1);

    let processed_count = client
        .query_one(
            "select count(*) from force_sync_outbox where processed_at is not null and outbox_id in ($1, $2)",
            &[&first_outbox_id, &second_outbox_id],
        )
        .await?;
    assert_eq!(processed_count.get::<_, i64>(0), 2);
    Ok(())
}

#[tokio::test]
#[ignore = "requires FORCE_SYNC_TEST_DATABASE_URL"]
async fn contradictory_op_and_tombstone_rows_are_rejected_safely() -> Result<(), ForceSyncError> {
    let pool = support::postgres::test_pool();
    support::postgres::reset_schema(&pool).await?;
    force_sync::migrate(&pool).await?;

    let payload = json!({"Name": "Acme"});
    assert!(
        insert_outbox_row(
            &pool,
            &OutboxSeed {
                tenant: "tenant",
                object_name: "Account",
                external_id: "external-contradict",
                source_cursor: "postgres-lsn-contradict",
                op: "upsert",
                tombstone: true,
                payload: &payload,
            },
        )
        .await
        .is_err()
    );

    let client = pool.get().await?;
    let journal_count = client
        .query_one(
            "select count(*) from sync_journal where external_id = $1",
            &[&"external-contradict"],
        )
        .await?;
    assert_eq!(journal_count.get::<_, i64>(0), 0);

    Ok(())
}

#[tokio::test]
#[ignore = "requires FORCE_SYNC_TEST_DATABASE_URL"]
async fn invalid_outbox_row_is_dead_lettered_and_marked_processed() -> Result<(), ForceSyncError> {
    let pool = support::postgres::test_pool();
    support::postgres::reset_schema(&pool).await?;
    force_sync::migrate(&pool).await?;

    let payload = json!({"Name": "Acme"});
    insert_outbox_row(
        &pool,
        &OutboxSeed {
            tenant: "",
            object_name: "Account",
            external_id: "external-invalid",
            source_cursor: "postgres-lsn-invalid",
            op: "upsert",
            tombstone: false,
            payload: &payload,
        },
    )
    .await?;

    let store = PgStore::new(pool.clone());
    assert_eq!(capture_batch(&store, 10, 25).await?, 1);

    let client = pool.get().await?;
    let journal_count = client
        .query_one(
            "select count(*) from sync_journal where external_id = $1",
            &[&"external-invalid"],
        )
        .await?;
    assert_eq!(journal_count.get::<_, i64>(0), 0);

    assert_eq!(dead_letter_count(&pool).await?, 1);
    assert_eq!(
        error_message_for_external_id(&pool, "external-invalid")
            .await?
            .as_deref(),
        Some("sync key tenant cannot be empty")
    );

    let processed_row = client
        .query_one(
            "select processed_at is not null from force_sync_outbox where external_id = $1",
            &[&"external-invalid"],
        )
        .await?;
    assert!(processed_row.get::<_, bool>(0));
    Ok(())
}

#[tokio::test]
#[ignore = "requires FORCE_SYNC_TEST_DATABASE_URL"]
async fn already_encoded_cursor_is_rejected_safely() -> Result<(), ForceSyncError> {
    let pool = support::postgres::test_pool();
    support::postgres::reset_schema(&pool).await?;
    force_sync::migrate(&pool).await?;

    let payload = json!({"Name": "Acme"});
    assert!(
        insert_outbox_row(
            &pool,
            &OutboxSeed {
                tenant: "tenant",
                object_name: "Account",
                external_id: "external-cursor",
                source_cursor: "postgres-lsn:already-encoded",
                op: "upsert",
                tombstone: false,
                payload: &payload,
            },
        )
        .await
        .is_err()
    );

    let client = pool.get().await?;
    let journal_count = client
        .query_one(
            "select count(*) from sync_journal where external_id = $1",
            &[&"external-cursor"],
        )
        .await?;
    assert_eq!(journal_count.get::<_, i64>(0), 0);

    Ok(())
}
