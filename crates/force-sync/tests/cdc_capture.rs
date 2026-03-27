//! Stream-driven Salesforce CDC capture tests.

mod support;

use force_pubsub::{EventMessage, PubSubEvent, ReplayId};
use futures::stream;
use serde_json::{Value, json};

use force_sync::{
    capture::salesforce::{capture_stream, load_replay_id},
    config::ObjectSync,
    error::ForceSyncError,
    store::pg::PgStore,
};

fn event(payload: Value, replay_id: &[u8], event_id: &str) -> PubSubEvent<Value> {
    PubSubEvent::Event(EventMessage {
        payload,
        replay_id: ReplayId::from_bytes(replay_id.to_vec()),
        schema_id: "schema-1".to_string(),
        event_id: event_id.to_string(),
    })
}

#[tokio::test]
#[ignore = "requires FORCE_SYNC_TEST_DATABASE_URL"]
async fn cdc_event_creates_journal_task_and_checkpoint() -> Result<(), ForceSyncError> {
    let pool = support::postgres::test_pool();
    support::postgres::reset_schema(&pool).await?;
    force_sync::store::pg::migrate(&pool).await?;
    let store = PgStore::new(pool.clone());
    let object = ObjectSync::new("Account").external_id("ExternalId__c");

    let stream = stream::iter(vec![Ok(event(
        json!({
            "ChangeEventHeader": {
                "entityName": "Account",
                "changeType": "UPDATE"
            },
            "ExternalId__c": "external-1",
            "Name": "Acme Corp"
        }),
        &[1],
        "evt-1",
    ))]);

    let captured = capture_stream(&store, "salesforce:Account", "tenant", &object, stream).await?;
    assert_eq!(captured, 1);

    let client = pool.get().await?;
    let journal = client
        .query_one(
            "select source, source_cursor, operation
             from sync_journal
             where tenant = 'tenant' and object_name = 'Account' and external_id = 'external-1'",
            &[],
        )
        .await?;
    assert_eq!(journal.get::<_, String>(0), "salesforce");
    assert_eq!(journal.get::<_, String>(1), "salesforce-replay-id:1");
    assert_eq!(journal.get::<_, String>(2), "upsert");

    let task_count = client
        .query_one(
            "select count(*) from sync_task where task_kind = 'apply'",
            &[],
        )
        .await?;
    assert_eq!(task_count.get::<_, i64>(0), 1);

    let checkpoint = client
        .query_one(
            "select cursor_position, cursor from sync_checkpoint where stream_name = 'salesforce:Account'",
            &[],
        )
        .await?;
    assert_eq!(checkpoint.get::<_, i64>(0), 1);
    assert_eq!(
        checkpoint.get::<_, Option<String>>(1).as_deref(),
        Some("salesforce-replay-id:1")
    );

    Ok(())
}

#[tokio::test]
#[ignore = "requires FORCE_SYNC_TEST_DATABASE_URL"]
async fn duplicate_cdc_event_is_deduped_by_replay_id() -> Result<(), ForceSyncError> {
    let pool = support::postgres::test_pool();
    support::postgres::reset_schema(&pool).await?;
    force_sync::store::pg::migrate(&pool).await?;
    let store = PgStore::new(pool.clone());
    let object = ObjectSync::new("Account").external_id("ExternalId__c");

    let payload = json!({
        "ChangeEventHeader": {
            "entityName": "Account",
            "changeType": "UPDATE"
        },
        "ExternalId__c": "external-1",
        "Name": "Acme Corp"
    });
    let stream = stream::iter(vec![
        Ok(event(payload.clone(), &[2], "evt-2a")),
        Ok(event(payload, &[2], "evt-2b")),
    ]);

    let captured = capture_stream(&store, "salesforce:Account", "tenant", &object, stream).await?;
    assert_eq!(captured, 1);

    let client = pool.get().await?;
    let journal_count = client
        .query_one(
            "select count(*) from sync_journal where source_cursor = 'salesforce-replay-id:2'",
            &[],
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

    Ok(())
}

#[tokio::test]
#[ignore = "requires FORCE_SYNC_TEST_DATABASE_URL"]
async fn restart_uses_the_stored_replay_cursor() -> Result<(), ForceSyncError> {
    let pool = support::postgres::test_pool();
    support::postgres::reset_schema(&pool).await?;
    force_sync::store::pg::migrate(&pool).await?;
    let store = PgStore::new(pool.clone());
    let object = ObjectSync::new("Account").external_id("ExternalId__c");

    let stream = stream::iter(vec![Ok(event(
        json!({
            "ChangeEventHeader": {
                "entityName": "Account",
                "changeType": "UPDATE"
            },
            "ExternalId__c": "external-2",
            "Name": "Acme Resume"
        }),
        &[9],
        "evt-9",
    ))]);
    let _ = capture_stream(&store, "salesforce:Account", "tenant", &object, stream).await?;

    let replay_id = load_replay_id(&store, "salesforce:Account").await?;
    assert_eq!(replay_id.as_ref().map(ReplayId::as_bytes), Some(&[9][..]));

    Ok(())
}
