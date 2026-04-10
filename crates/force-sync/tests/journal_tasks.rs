//! Integration tests for journal writes and task leasing.

mod support;

use std::time::Duration;

use chrono::Utc;
use futures::FutureExt;
use serde_json::json;

use force_sync::{
    SyncKey,
    ChangeEnvelope, ChangeOperation, SourceCursor, SourceSystem,
    AppendResult,
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
async fn appending_a_journal_entry_creates_a_row() -> Result<(), force_sync::ForceSyncError>
{
    let pool = support::postgres::test_pool();
    support::postgres::reset_schema(&pool).await?;
    force_sync::migrate(&pool).await?;

    let store = force_sync::PgStore::new(pool.clone());
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
async fn duplicate_source_cursor_is_deduped() -> Result<(), force_sync::ForceSyncError> {
    let pool = support::postgres::test_pool();
    support::postgres::reset_schema(&pool).await?;
    force_sync::migrate(&pool).await?;

    let store = force_sync::PgStore::new(pool.clone());
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
-> Result<(), force_sync::ForceSyncError> {
    let pool = support::postgres::test_pool();
    support::postgres::reset_schema(&pool).await?;
    force_sync::migrate(&pool).await?;

    let store = force_sync::PgStore::new(pool.clone());
    let envelope = test_envelope(3);

    let journal_id = store
        .with_transaction(|tx| {
            async move {
                let journal_id =
                    force_sync::PgStore::append_journal_in_tx(tx, &envelope).await?;
                force_sync::PgStore::enqueue_apply_task_in_tx(tx, journal_id, 10)
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
async fn leasing_a_task_marks_owner_and_until() -> Result<(), force_sync::ForceSyncError> {
    let pool = support::postgres::test_pool();
    support::postgres::reset_schema(&pool).await?;
    force_sync::migrate(&pool).await?;

    let store = force_sync::PgStore::new(pool.clone());
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
-> Result<(), force_sync::ForceSyncError> {
    let pool = support::postgres::test_pool();
    support::postgres::reset_schema(&pool).await?;
    force_sync::migrate(&pool).await?;

    let store = force_sync::PgStore::new(pool.clone());
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

#[tokio::test]
#[ignore = "requires FORCE_SYNC_TEST_DATABASE_URL"]
async fn wrong_worker_cannot_ack_task() -> Result<(), force_sync::ForceSyncError> {
    let pool = support::postgres::test_pool();
    support::postgres::reset_schema(&pool).await?;
    force_sync::migrate(&pool).await?;

    let store = force_sync::PgStore::new(pool.clone());
    let envelope = test_envelope(10);
    let journal_id = store.append_journal(&envelope).await?;
    store.enqueue_apply_task(journal_id, 5).await?;

    let leased = store
        .lease_ready_tasks("worker-A", 1, Duration::from_secs(60))
        .await?;
    assert_eq!(leased.len(), 1);

    // Wrong worker tries to ack -- should affect 0 rows.
    let rows_affected = store
        .ack_task_for_worker("worker-B", leased[0].task_id)
        .await?;
    assert_eq!(rows_affected, 0);

    // Correct worker succeeds.
    let rows_affected = store
        .ack_task_for_worker("worker-A", leased[0].task_id)
        .await?;
    assert_eq!(rows_affected, 1);
    Ok(())
}

#[tokio::test]
#[ignore = "requires FORCE_SYNC_TEST_DATABASE_URL"]
async fn wrong_worker_cannot_fail_task() -> Result<(), force_sync::ForceSyncError> {
    let pool = support::postgres::test_pool();
    support::postgres::reset_schema(&pool).await?;
    force_sync::migrate(&pool).await?;

    let store = force_sync::PgStore::new(pool.clone());
    let envelope = test_envelope(11);
    let journal_id = store.append_journal(&envelope).await?;
    store.enqueue_apply_task(journal_id, 5).await?;

    let leased = store
        .lease_ready_tasks("worker-A", 1, Duration::from_secs(60))
        .await?;
    assert_eq!(leased.len(), 1);

    // Wrong worker tries to fail -- should affect 0 rows.
    let rows_affected = store
        .fail_task_for_worker("worker-B", leased[0].task_id, "wrong worker error")
        .await?;
    assert_eq!(rows_affected, 0);

    // Correct worker succeeds.
    let rows_affected = store
        .fail_task_for_worker("worker-A", leased[0].task_id, "real error")
        .await?;
    assert_eq!(rows_affected, 1);

    let row = pool
        .get()
        .await?
        .query_one(
            "select status, last_error from sync_task where task_id = $1",
            &[&leased[0].task_id],
        )
        .await?;
    assert_eq!(row.get::<_, String>(0), "failed");
    assert_eq!(
        row.get::<_, Option<String>>(1),
        Some("real error".to_owned())
    );
    Ok(())
}

#[tokio::test]
#[ignore = "requires FORCE_SYNC_TEST_DATABASE_URL"]
async fn retry_task_with_future_next_attempt_at_is_not_leasable_until_due()
-> Result<(), force_sync::ForceSyncError> {
    let pool = support::postgres::test_pool();
    support::postgres::reset_schema(&pool).await?;
    force_sync::migrate(&pool).await?;

    let store = force_sync::PgStore::new(pool.clone());
    let envelope = test_envelope(12);
    let journal_id = store.append_journal(&envelope).await?;
    store.enqueue_apply_task(journal_id, 5).await?;

    let leased = store
        .lease_ready_tasks("worker-1", 1, Duration::from_secs(60))
        .await?;
    assert_eq!(leased.len(), 1);

    // Schedule retry 10 minutes in the future.
    let future_retry = Utc::now() + chrono::Duration::minutes(10);
    let rows_affected = store
        .retry_task(leased[0].task_id, future_retry, "transient error")
        .await?;
    assert_eq!(rows_affected, 1);

    // Task should not be leasable because next_attempt_at is in the future.
    let available = store
        .lease_ready_tasks("worker-1", 10, Duration::from_secs(60))
        .await?;
    assert!(available.is_empty());

    // Manually set next_attempt_at to the past so we can re-lease.
    let past = Utc::now() - chrono::Duration::seconds(1);
    store
        .retry_task(leased[0].task_id, past, "transient error")
        .await?;

    let available = store
        .lease_ready_tasks("worker-1", 10, Duration::from_secs(60))
        .await?;
    assert_eq!(available.len(), 1);
    assert_eq!(available[0].task_id, leased[0].task_id);
    Ok(())
}

#[tokio::test]
#[ignore = "requires FORCE_SYNC_TEST_DATABASE_URL"]
async fn done_task_cannot_be_re_leased() -> Result<(), force_sync::ForceSyncError> {
    let pool = support::postgres::test_pool();
    support::postgres::reset_schema(&pool).await?;
    force_sync::migrate(&pool).await?;

    let store = force_sync::PgStore::new(pool.clone());
    let envelope = test_envelope(13);
    let journal_id = store.append_journal(&envelope).await?;
    store.enqueue_apply_task(journal_id, 5).await?;

    let leased = store
        .lease_ready_tasks("worker-1", 1, Duration::from_secs(60))
        .await?;
    assert_eq!(leased.len(), 1);

    // Mark as done.
    store.ack_task(leased[0].task_id).await?;

    // Attempting to lease again should return nothing -- done tasks stay done.
    let leased_again = store
        .lease_ready_tasks("worker-1", 10, Duration::from_secs(60))
        .await?;
    assert!(leased_again.is_empty());
    Ok(())
}

#[tokio::test]
#[ignore = "requires FORCE_SYNC_TEST_DATABASE_URL"]
async fn in_tx_lease_and_ack_round_trip() -> Result<(), force_sync::ForceSyncError> {
    let pool = support::postgres::test_pool();
    support::postgres::reset_schema(&pool).await?;
    force_sync::migrate(&pool).await?;

    let store = force_sync::PgStore::new(pool.clone());
    let envelope = test_envelope(14);

    // Enqueue inside a transaction using _in_tx variant.
    let journal_id = store
        .with_transaction(|tx| {
            async move {
                let jid =
                    force_sync::PgStore::append_journal_in_tx(tx, &envelope).await?;
                force_sync::PgStore::enqueue_apply_task_in_tx(tx, jid, 10).await?;
                Ok(jid)
            }
            .boxed()
        })
        .await?;

    // Lease inside a transaction using _in_tx variant.
    let leased = store
        .with_transaction(|tx| {
            async move {
                let tasks = force_sync::PgStore::lease_ready_tasks_in_tx(
                    tx,
                    "tx-worker",
                    1,
                    Duration::from_secs(60),
                )
                .await?;
                Ok(tasks)
            }
            .boxed()
        })
        .await?;
    assert_eq!(leased.len(), 1);
    assert_eq!(leased[0].lease_owner, "tx-worker");

    // Ack inside a transaction using _in_tx variant.
    let rows_affected = store
        .with_transaction(|tx| {
            let task_id = leased[0].task_id;
            async move { force_sync::PgStore::ack_task_in_tx(tx, task_id).await }.boxed()
        })
        .await?;
    assert_eq!(rows_affected, 1);

    // Verify status is done.
    let row = pool
        .get()
        .await?
        .query_one(
            "select status from sync_task where target_key = $1",
            &[&journal_id.to_string()],
        )
        .await?;
    assert_eq!(row.get::<_, String>(0), "done");
    Ok(())
}

#[tokio::test]
#[ignore = "requires FORCE_SYNC_TEST_DATABASE_URL"]
async fn in_tx_retry_and_fail_round_trip() -> Result<(), force_sync::ForceSyncError> {
    let pool = support::postgres::test_pool();
    support::postgres::reset_schema(&pool).await?;
    force_sync::migrate(&pool).await?;

    let store = force_sync::PgStore::new(pool.clone());
    let envelope = test_envelope(15);
    let journal_id = store.append_journal(&envelope).await?;
    store.enqueue_apply_task(journal_id, 5).await?;

    let leased = store
        .lease_ready_tasks("w1", 1, Duration::from_secs(60))
        .await?;
    assert_eq!(leased.len(), 1);
    let task_id = leased[0].task_id;

    // Retry inside a transaction using _in_tx variant.
    let past = Utc::now() - chrono::Duration::seconds(1);
    let rows_affected = store
        .with_transaction(|tx| {
            async move {
                force_sync::PgStore::retry_task_in_tx(tx, task_id, past, "oops").await
            }
            .boxed()
        })
        .await?;
    assert_eq!(rows_affected, 1);

    // Re-lease and fail inside a transaction.
    let leased = store
        .lease_ready_tasks("w1", 1, Duration::from_secs(60))
        .await?;
    assert_eq!(leased.len(), 1);

    let fail_task_id = leased[0].task_id;
    let rows_affected = store
        .with_transaction(|tx| {
            async move {
                force_sync::PgStore::fail_task_in_tx(tx, fail_task_id, "fatal").await
            }
            .boxed()
        })
        .await?;
    assert_eq!(rows_affected, 1);

    let row = pool
        .get()
        .await?
        .query_one(
            "select status, last_error from sync_task where task_id = $1",
            &[&task_id],
        )
        .await?;
    assert_eq!(row.get::<_, String>(0), "failed");
    assert_eq!(row.get::<_, Option<String>>(1), Some("fatal".to_owned()));
    Ok(())
}

#[tokio::test]
#[ignore = "requires FORCE_SYNC_TEST_DATABASE_URL"]
async fn failed_task_cannot_be_re_leased() -> Result<(), force_sync::ForceSyncError> {
    let pool = support::postgres::test_pool();
    support::postgres::reset_schema(&pool).await?;
    force_sync::migrate(&pool).await?;

    let store = force_sync::PgStore::new(pool.clone());
    let envelope = test_envelope(16);
    let journal_id = store.append_journal(&envelope).await?;
    store.enqueue_apply_task(journal_id, 5).await?;

    let leased = store
        .lease_ready_tasks("worker-1", 1, Duration::from_secs(60))
        .await?;
    assert_eq!(leased.len(), 1);

    // Mark as failed.
    store
        .fail_task(leased[0].task_id, "permanent error")
        .await?;

    // Failed tasks should not be leasable.
    let leased_again = store
        .lease_ready_tasks("worker-1", 10, Duration::from_secs(60))
        .await?;
    assert!(leased_again.is_empty());
    Ok(())
}
