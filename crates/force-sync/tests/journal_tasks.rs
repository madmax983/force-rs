//! Integration tests for journal writes and task leasing.

mod support;

use std::time::Duration;

use chrono::Utc;
use futures::FutureExt;
use serde_json::json;

use force_sync::{
    identity::SyncKey,
    model::{ChangeEnvelope, ChangeOperation, SourceCursor, SourceSystem},
    store::pg::AppendResult,
};

fn test_envelope(cursor: i64) -> ChangeEnvelope {
    let sync_key = match SyncKey::new("tenant", "Account", format!("external-{cursor}")) {
        Ok(sync_key) => sync_key,
        Err(error) => panic!("unexpected sync key construction error: {error}"),
    };

    ChangeEnvelope::new(
        sync_key,
        SourceSystem::Salesforce,
        ChangeOperation::Upsert,
        Utc::now(),
        json!({
            "Name": "Acme",
            "Cursor": cursor
        }),
    )
    .with_cursor(SourceCursor::SalesforceReplayId(cursor))
}

#[tokio::test]
#[ignore = "requires FORCE_SYNC_TEST_DATABASE_URL"]
async fn appending_a_journal_entry_creates_a_row() -> Result<(), force_sync::error::ForceSyncError>
{
    let pool = support::postgres::test_pool();
    support::postgres::reset_schema(&pool).await?;
    force_sync::store::pg::migrate(&pool).await?;

    let store = force_sync::store::pg::PgStore::new(pool.clone());
    let envelope = test_envelope(1);

    let journal_id = store.append_journal(&envelope).await?;
    let client = pool.get().await?;
    let row = client
        .query_one(
            "select tenant, object_name, external_id, source, source_cursor, operation, tombstone
             from sync_journal
             where journal_id = $1",
            &[&journal_id],
        )
        .await?;

    assert_eq!(row.get::<_, String>(0), "tenant");
    assert_eq!(row.get::<_, String>(1), "Account");
    assert_eq!(row.get::<_, String>(2), "external-1");
    assert_eq!(row.get::<_, String>(3), "salesforce");
    assert_eq!(row.get::<_, String>(4), "salesforce-replay-id:1");
    assert_eq!(row.get::<_, String>(5), "upsert");
    assert!(!row.get::<_, bool>(6));
    Ok(())
}

#[tokio::test]
#[ignore = "requires FORCE_SYNC_TEST_DATABASE_URL"]
async fn duplicate_source_cursor_is_deduped() -> Result<(), force_sync::error::ForceSyncError> {
    let pool = support::postgres::test_pool();
    support::postgres::reset_schema(&pool).await?;
    force_sync::store::pg::migrate(&pool).await?;

    let store = force_sync::store::pg::PgStore::new(pool.clone());
    let envelope = test_envelope(2);

    match store.append_journal_if_new(&envelope).await? {
        AppendResult::Inserted { journal_id } => {
            assert!(journal_id > 0);
        }
        AppendResult::Duplicate => panic!("first insert should succeed"),
    }

    assert!(matches!(
        store.append_journal_if_new(&envelope).await?,
        AppendResult::Duplicate
    ));

    let row = pool
        .get()
        .await?
        .query_one("select count(*) from sync_journal", &[])
        .await?;
    assert_eq!(row.get::<_, i64>(0), 1);
    Ok(())
}

#[tokio::test]
#[ignore = "requires FORCE_SYNC_TEST_DATABASE_URL"]
async fn enqueuing_a_task_in_the_same_transaction_works()
-> Result<(), force_sync::error::ForceSyncError> {
    let pool = support::postgres::test_pool();
    support::postgres::reset_schema(&pool).await?;
    force_sync::store::pg::migrate(&pool).await?;

    let store = force_sync::store::pg::PgStore::new(pool.clone());
    let envelope = test_envelope(3);

    let journal_id = store
        .with_transaction(|tx| {
            async move {
                let journal_id =
                    force_sync::store::pg::PgStore::append_journal_in_tx(tx, &envelope).await?;
                force_sync::store::pg::PgStore::enqueue_apply_task_in_tx(tx, journal_id, 10)
                    .await?;
                Ok(journal_id)
            }
            .boxed()
        })
        .await?;

    let row = pool
        .get()
        .await?
        .query_one(
            "select count(*) from sync_task where target_key = $1",
            &[&journal_id.to_string()],
        )
        .await?;
    assert_eq!(row.get::<_, i64>(0), 1);
    Ok(())
}

#[tokio::test]
#[ignore = "requires FORCE_SYNC_TEST_DATABASE_URL"]
async fn leasing_a_task_marks_owner_and_until() -> Result<(), force_sync::error::ForceSyncError> {
    let pool = support::postgres::test_pool();
    support::postgres::reset_schema(&pool).await?;
    force_sync::store::pg::migrate(&pool).await?;

    let store = force_sync::store::pg::PgStore::new(pool.clone());
    let envelope = test_envelope(4);
    let journal_id = store.append_journal(&envelope).await?;
    store.enqueue_apply_task(journal_id, 5).await?;

    let leased = store
        .lease_ready_tasks("worker-1", 10, Duration::from_secs(60))
        .await?;
    assert_eq!(leased.len(), 1);
    assert_eq!(leased[0].lease_owner, "worker-1");
    assert!(leased[0].lease_until > Utc::now());

    let row = pool
        .get()
        .await?
        .query_one(
            "select lease_owner, lease_until is not null from sync_task where task_id = $1",
            &[&leased[0].task_id],
        )
        .await?;
    assert_eq!(row.get::<_, String>(0), "worker-1");
    assert!(row.get::<_, bool>(1));
    Ok(())
}

#[tokio::test]
#[ignore = "requires FORCE_SYNC_TEST_DATABASE_URL"]
async fn worker_guarded_task_updates_require_the_current_lease_and_clear_retry_state()
-> Result<(), force_sync::error::ForceSyncError> {
    let pool = support::postgres::test_pool();
    support::postgres::reset_schema(&pool).await?;
    force_sync::store::pg::migrate(&pool).await?;

    let store = force_sync::store::pg::PgStore::new(pool.clone());
    let envelope = test_envelope(5);
    let journal_id = store.append_journal(&envelope).await?;
    store.enqueue_apply_task(journal_id, 5).await?;

    let leased = store
        .lease_ready_tasks("worker-1", 1, Duration::from_secs(60))
        .await?;
    assert_eq!(leased.len(), 1);

    assert_eq!(
        store
            .ack_task_for_worker("worker-2", leased[0].task_id)
            .await?,
        0
    );

    let retry_at = Utc::now() + chrono::Duration::minutes(5);
    assert_eq!(
        store
            .retry_task_for_worker("worker-1", leased[0].task_id, retry_at, "boom")
            .await?,
        1
    );

    let not_ready_yet = store
        .lease_ready_tasks("worker-1", 1, Duration::from_secs(60))
        .await?;
    assert!(not_ready_yet.is_empty());

    assert_eq!(
        store
            .retry_task(
                leased[0].task_id,
                Utc::now() - chrono::Duration::seconds(1),
                "boom"
            )
            .await?,
        1
    );

    let leased_again = store
        .lease_ready_tasks("worker-1", 1, Duration::from_secs(60))
        .await?;
    assert_eq!(leased_again.len(), 1);
    assert_eq!(leased_again[0].task_id, leased[0].task_id);

    assert_eq!(
        store
            .ack_task_for_worker("worker-1", leased_again[0].task_id)
            .await?,
        1
    );

    let row = pool
        .get()
        .await?
        .query_one(
            "select status, last_error, next_attempt_at is not null
             from sync_task
             where task_id = $1",
            &[&leased_again[0].task_id],
        )
        .await?;
    assert_eq!(row.get::<_, String>(0), "done");
    assert_eq!(row.get::<_, Option<String>>(1), None);
    assert!(row.get::<_, bool>(2));
    Ok(())
}
