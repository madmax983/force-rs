//! Integration tests for link, checkpoint, conflict, and dead-letter repositories.

mod support;

use serde_json::{Value, json};

use force_sync::{DeadLetter, ForceSyncError, SyncConflict, SyncLink};

#[tokio::test]
#[ignore = "requires FORCE_SYNC_TEST_DATABASE_URL"]
async fn put_link_upserts_the_latest_values() -> Result<(), force_sync::ForceSyncError> {
    let pool = support::postgres::test_pool();
    support::postgres::reset_schema(&pool).await?;
    force_sync::migrate(&pool).await?;

    let store = force_sync::PgStore::new(pool.clone());

    let first = SyncLink {
        tenant: "tenant".to_string(),
        object_name: "Account".to_string(),
        external_id: "external-1".to_string(),
        salesforce_id: Some("sf-1".to_string()),
        postgres_id: Some("pg-1".to_string()),
        last_source: Some("salesforce".to_string()),
        last_source_cursor: Some("salesforce-replay-id:10".to_string()),
        last_payload_hash: Some(vec![1, 2, 3]),
        tombstone: false,
    };

    let second = SyncLink {
        salesforce_id: Some("sf-2".to_string()),
        postgres_id: Some("pg-2".to_string()),
        last_source_cursor: Some("salesforce-replay-id:11".to_string()),
        last_payload_hash: Some(vec![4, 5, 6]),
        tombstone: true,
        ..first.clone()
    };

    store.put_link(&first).await?;
    store.put_link(&second).await?;

    let loaded = store
        .get_link("tenant", "Account", "external-1")
        .await?
        .ok_or(ForceSyncError::NotFound {
            entity: "sync_link",
        })?;
    assert_eq!(loaded.tenant, "tenant");
    assert_eq!(loaded.object_name, "Account");
    assert_eq!(loaded.external_id, "external-1");
    assert_eq!(loaded.salesforce_id.as_deref(), Some("sf-2"));
    assert_eq!(loaded.postgres_id.as_deref(), Some("pg-2"));
    assert_eq!(loaded.last_source.as_deref(), Some("salesforce"));
    assert_eq!(
        loaded.last_source_cursor.as_deref(),
        Some("salesforce-replay-id:11")
    );
    assert_eq!(loaded.last_payload_hash, Some(vec![4, 5, 6]));
    assert!(loaded.tombstone);
    Ok(())
}

#[tokio::test]
#[ignore = "requires FORCE_SYNC_TEST_DATABASE_URL"]
async fn checkpoint_advances_only_when_the_position_increases() -> Result<(), force_sync::ForceSyncError> {
    let pool = support::postgres::test_pool();
    support::postgres::reset_schema(&pool).await?;
    force_sync::migrate(&pool).await?;

    let store = force_sync::PgStore::new(pool.clone());

    assert_eq!(
        store
            .advance_checkpoint_if_greater("salesforce:Account", 10, "cursor-10")
            .await?,
        1
    );
    assert_eq!(
        store
            .advance_checkpoint_if_greater("salesforce:Account", 5, "cursor-5")
            .await?,
        0
    );
    assert_eq!(
        store
            .advance_checkpoint_if_greater("salesforce:Account", 15, "cursor-15")
            .await?,
        1
    );

    let row = pool
        .get()
        .await?
        .query_one(
            "select cursor_position, cursor from sync_checkpoint where stream_name = $1",
            &[&"salesforce:Account"],
        )
        .await?;
    assert_eq!(row.get::<_, i64>(0), 15);
    assert_eq!(row.get::<_, String>(1), "cursor-15");
    Ok(())
}

#[tokio::test]
#[ignore = "requires FORCE_SYNC_TEST_DATABASE_URL"]
async fn insert_conflict_writes_a_row() -> Result<(), force_sync::ForceSyncError> {
    let pool = support::postgres::test_pool();
    support::postgres::reset_schema(&pool).await?;
    force_sync::migrate(&pool).await?;

    let store = force_sync::PgStore::new(pool.clone());
    let conflict = SyncConflict {
        tenant: "tenant".to_string(),
        object_name: "Account".to_string(),
        external_id: "external-1".to_string(),
        field_name: "Name".to_string(),
        left_value: json!("Left"),
        right_value: json!("Right"),
        resolution: Some("manual".to_string()),
    };

    let conflict_id = store.insert_conflict(&conflict).await?;

    let row = pool
        .get()
        .await?
        .query_one(
            "select tenant, object_name, external_id, field_name, left_value, right_value, resolution
             from sync_conflict
             where conflict_id = $1",
            &[&conflict_id],
        )
        .await?;
    assert_eq!(row.get::<_, String>(0), "tenant");
    assert_eq!(row.get::<_, String>(1), "Account");
    assert_eq!(row.get::<_, String>(2), "external-1");
    assert_eq!(row.get::<_, String>(3), "Name");
    assert_eq!(row.get::<_, Value>(4), json!("Left"));
    assert_eq!(row.get::<_, Value>(5), json!("Right"));
    assert_eq!(row.get::<_, Option<String>>(6).as_deref(), Some("manual"));
    Ok(())
}

#[tokio::test]
#[ignore = "requires FORCE_SYNC_TEST_DATABASE_URL"]
async fn insert_dead_letter_writes_a_row() -> Result<(), force_sync::ForceSyncError> {
    let pool = support::postgres::test_pool();
    support::postgres::reset_schema(&pool).await?;
    force_sync::migrate(&pool).await?;

    let store = force_sync::PgStore::new(pool.clone());
    let dead_letter = DeadLetter {
        task_id: Some(42),
        tenant: Some("tenant".to_string()),
        object_name: Some("Account".to_string()),
        external_id: Some("external-1".to_string()),
        error_message: "boom".to_string(),
        payload: Some(json!({"task": "failed"})),
    };

    let dead_letter_id = store.insert_dead_letter(&dead_letter).await?;

    let row = pool
        .get()
        .await?
        .query_one(
            "select task_id, tenant, object_name, external_id, error_message, payload
             from sync_dead_letter
             where dead_letter_id = $1",
            &[&dead_letter_id],
        )
        .await?;
    assert_eq!(row.get::<_, Option<i64>>(0), Some(42));
    assert_eq!(row.get::<_, Option<String>>(1).as_deref(), Some("tenant"));
    assert_eq!(row.get::<_, Option<String>>(2).as_deref(), Some("Account"));
    assert_eq!(
        row.get::<_, Option<String>>(3).as_deref(),
        Some("external-1")
    );
    assert_eq!(row.get::<_, String>(4), "boom");
    assert_eq!(
        row.get::<_, Option<Value>>(5),
        Some(json!({"task": "failed"}))
    );
    Ok(())
}
