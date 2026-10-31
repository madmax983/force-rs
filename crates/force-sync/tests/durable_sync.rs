//! End-to-end tests for durable sync on a real harvest worker and scheduler.
//!
//! Each test resets the database, applies the force-sync and harvest
//! migrations, starts [`DurableSync`], and lets the pump schedule drive an
//! outbox row to a wiremock Salesforce. No test calls a `*_once` method.

// The CDC stream item type is fixed by force-pubsub, and its error is large.
#![allow(clippy::result_large_err)]

mod support;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use autumn_harvest::policy::RetryPolicy;
use force::{
    auth::{AccessToken, Authenticator, TokenResponse},
    client::{ForceClient, builder},
    error::Result as ForceResult,
};
use serde_json::json;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path, path_regex},
};

use force_pubsub::{EventMessage, PubSubEvent, ReplayId, ReplayPreset};
use futures::{StreamExt, stream};
use serde_json::Value;

use force_sync::durable::{
    DurableSync, DurableSyncConfig, DurableSyncRuntime, harvest_pool, migrate_harvest,
};
use force_sync::{
    CdcEventStream, CdcSource, CdcStart, CdcStream, CdcWindow, ForceSyncError, ObjectSync, PgStore,
    SyncEngine,
};

const UPSERT_PATH: &str = "/services/data/v68.0/sobjects/Account/ExternalId__c/external-1";
const SF_ID: &str = "001000000000001AAA";

#[derive(Debug, Clone)]
struct MockAuthenticator {
    instance_url: String,
}

#[async_trait]
impl Authenticator for MockAuthenticator {
    async fn authenticate(&self) -> ForceResult<AccessToken> {
        Ok(AccessToken::from_response(TokenResponse {
            access_token: secrecy::SecretString::new("test_token".to_owned().into()),
            instance_url: self.instance_url.clone(),
            token_type: "Bearer".to_owned(),
            issued_at: "1704067200000".to_owned(),
            signature: "test_sig".to_owned(),
            expires_in: Some(7200),
            refresh_token: None,
        }))
    }

    async fn refresh(&self) -> ForceResult<AccessToken> {
        self.authenticate().await
    }
}

async fn test_client(mock_server: &MockServer) -> ForceClient<MockAuthenticator> {
    builder()
        .authenticate(MockAuthenticator {
            instance_url: mock_server.uri(),
        })
        .build()
        .await
        .unwrap_or_else(|error| panic!("unexpected client build error: {error}"))
}

fn database_url() -> String {
    std::env::var("FORCE_SYNC_TEST_DATABASE_URL")
        .unwrap_or_else(|_| panic!("FORCE_SYNC_TEST_DATABASE_URL must be set"))
}

fn created() -> ResponseTemplate {
    ResponseTemplate::new(201).set_body_json(json!({
        "id": SF_ID,
        "success": true,
        "created": true,
        "errors": []
    }))
}

fn unavailable() -> ResponseTemplate {
    ResponseTemplate::new(503).set_body_json(json!([{
        "message": "service unavailable",
        "errorCode": "SERVER_UNAVAILABLE"
    }]))
}

/// A config that pumps every second and retries the Salesforce call fast.
fn fast_config(max_attempts: u32) -> DurableSyncConfig {
    DurableSyncConfig {
        pump_interval: Duration::from_secs(1),
        reconcile_interval: None,
        remote_retry: RetryPolicy {
            jitter: autumn_harvest::policy::JitterPolicy::None,
            ..RetryPolicy::fixed(max_attempts, Duration::from_secs(1))
        },
        ..DurableSyncConfig::default()
    }
}

struct Harness {
    pool: deadpool_postgres::Pool,
    mock_server: MockServer,
}

impl Harness {
    async fn new() -> Result<Self, ForceSyncError> {
        let pool = support::postgres::test_pool();
        support::postgres::reset_schema(&pool).await?;
        force_sync::migrate(&pool).await?;
        migrate_harvest(&database_url()).await?;
        Ok(Self {
            pool,
            mock_server: MockServer::start().await,
        })
    }

    async fn start(&self, config: DurableSyncConfig) -> Result<DurableSyncRuntime, ForceSyncError> {
        let engine = SyncEngine::builder(test_client(&self.mock_server).await)
            .postgres(PgStore::new(self.pool.clone()))
            .object(ObjectSync::new("Account").external_id("ExternalId__c"))
            .build()?;
        DurableSync::new(engine, harvest_pool(&database_url(), 8)?)
            .with_config(config)?
            .start()
    }

    async fn start_with_cdc(
        &self,
        config: DurableSyncConfig,
        source: Arc<dyn CdcSource>,
    ) -> Result<DurableSyncRuntime, ForceSyncError> {
        let engine = SyncEngine::builder(test_client(&self.mock_server).await)
            .postgres(PgStore::new(self.pool.clone()))
            .object(ObjectSync::new("Account").external_id("ExternalId__c"))
            .build()?;
        DurableSync::new(engine, harvest_pool(&database_url(), 8)?)
            .with_config(config)?
            .with_cdc(
                source,
                [CdcStream::new("tenant", "Account").starting_at(CdcStart::Earliest)],
            )?
            .start()
    }

    async fn insert_outbox_row(&self, cursor: &str, name: &str) -> Result<(), ForceSyncError> {
        self.pool
            .get()
            .await?
            .execute(
                "insert into force_sync_outbox
                    (tenant, object_name, external_id, source_cursor, op, tombstone, payload)
                 values ('tenant', 'Account', 'external-1', $1, 'upsert', false, $2::jsonb)",
                &[&cursor, &json!({ "Name": name })],
            )
            .await?;
        Ok(())
    }

    /// `(status, last_error)` of every apply task, in task order.
    async fn tasks(&self) -> Result<Vec<(String, Option<String>)>, ForceSyncError> {
        Ok(self
            .pool
            .get()
            .await?
            .query(
                "select status, last_error from sync_task
                 where task_kind = 'apply' order by task_id",
                &[],
            )
            .await?
            .iter()
            .map(|row| (row.get(0), row.get(1)))
            .collect())
    }

    /// Waits until `count` apply tasks exist and none is ready or leased.
    async fn wait_for_settled(
        &self,
        count: usize,
    ) -> Result<Vec<(String, Option<String>)>, ForceSyncError> {
        let deadline = Instant::now() + Duration::from_secs(90);
        loop {
            let tasks = self.tasks().await?;
            let settled = tasks.len() == count
                && tasks
                    .iter()
                    .all(|(status, _)| status == "done" || status == "failed");
            if settled {
                return Ok(tasks);
            }
            assert!(
                Instant::now() < deadline,
                "tasks did not settle in time: {tasks:?}"
            );
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }

    async fn salesforce_calls(&self) -> usize {
        self.mock_server
            .received_requests()
            .await
            .map_or(0, |requests| requests.len())
    }

    async fn linked_salesforce_id(&self) -> Result<Option<String>, ForceSyncError> {
        Ok(self
            .pool
            .get()
            .await?
            .query_opt(
                "select salesforce_id from sync_link
                 where tenant = 'tenant' and object_name = 'Account'
                   and external_id = 'external-1'",
                &[],
            )
            .await?
            .and_then(|row| row.get(0)))
    }
}

#[tokio::test]
#[ignore = "requires FORCE_SYNC_TEST_DATABASE_URL"]
async fn pump_schedule_drives_an_outbox_row_to_salesforce() -> Result<(), ForceSyncError> {
    let harness = Harness::new().await?;
    Mock::given(method("PATCH"))
        .and(path(UPSERT_PATH))
        .respond_with(created())
        .expect(1)
        .mount(&harness.mock_server)
        .await;

    harness.insert_outbox_row("lsn-1", "Acme Corp").await?;
    let runtime = harness.start(fast_config(3)).await?;
    let tasks = harness.wait_for_settled(1).await?;
    runtime.stop().await?;

    assert_eq!(tasks, vec![("done".to_owned(), None)]);
    assert_eq!(
        harness.linked_salesforce_id().await?.as_deref(),
        Some(SF_ID)
    );
    assert_eq!(harness.salesforce_calls().await, 1);
    Ok(())
}

#[tokio::test]
#[ignore = "requires FORCE_SYNC_TEST_DATABASE_URL"]
async fn transient_salesforce_errors_are_retried_by_harvest() -> Result<(), ForceSyncError> {
    let harness = Harness::new().await?;
    Mock::given(method("PATCH"))
        .and(path(UPSERT_PATH))
        .respond_with(unavailable())
        .up_to_n_times(2)
        .with_priority(1)
        .mount(&harness.mock_server)
        .await;
    Mock::given(method("PATCH"))
        .and(path(UPSERT_PATH))
        .respond_with(created())
        .with_priority(2)
        .mount(&harness.mock_server)
        .await;

    harness.insert_outbox_row("lsn-1", "Acme Corp").await?;
    let runtime = harness
        // The force client also retries a 503 inside one attempt, so allow
        // enough attempts for both layers.
        .start(fast_config(5))
        .await?;
    let tasks = harness.wait_for_settled(1).await?;
    runtime.stop().await?;

    assert_eq!(tasks, vec![("done".to_owned(), None)]);
    assert_eq!(
        harness.linked_salesforce_id().await?.as_deref(),
        Some(SF_ID)
    );
    assert!(harness.salesforce_calls().await >= 3);
    Ok(())
}

#[tokio::test]
#[ignore = "requires FORCE_SYNC_TEST_DATABASE_URL"]
async fn exhausted_retries_fail_the_task_after_the_configured_attempts()
-> Result<(), ForceSyncError> {
    let harness = Harness::new().await?;
    Mock::given(method("PATCH"))
        .and(path(UPSERT_PATH))
        .respond_with(unavailable())
        .mount(&harness.mock_server)
        .await;

    harness.insert_outbox_row("lsn-1", "Acme Corp").await?;
    let runtime = harness.start(fast_config(2)).await?;
    let tasks = harness.wait_for_settled(1).await?;
    let calls_at_settle = harness.salesforce_calls().await;
    tokio::time::sleep(Duration::from_secs(3)).await;
    let calls_after = harness.salesforce_calls().await;
    runtime.stop().await?;

    let [(status, Some(error))] = tasks.as_slice() else {
        panic!("expected one failed task with an error, got {tasks:?}");
    };
    assert_eq!(status, "failed");
    assert!(error.contains("retries exhausted"), "{error}");
    assert_eq!(
        calls_after, calls_at_settle,
        "a failed task must not call Salesforce again"
    );
    assert_eq!(harness.linked_salesforce_id().await?, None);
    Ok(())
}

#[tokio::test]
#[ignore = "requires FORCE_SYNC_TEST_DATABASE_URL"]
async fn rejected_write_fails_the_task_without_retry() -> Result<(), ForceSyncError> {
    let harness = Harness::new().await?;
    Mock::given(method("PATCH"))
        .and(path(UPSERT_PATH))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!([{
            "message": "Name: bad value",
            "errorCode": "INVALID_FIELD"
        }])))
        .expect(1)
        .mount(&harness.mock_server)
        .await;

    harness.insert_outbox_row("lsn-1", "Acme Corp").await?;
    let runtime = harness.start(fast_config(5)).await?;
    let tasks = harness.wait_for_settled(1).await?;
    runtime.stop().await?;

    let [(status, Some(error))] = tasks.as_slice() else {
        panic!("expected one failed task with an error, got {tasks:?}");
    };
    assert_eq!(status, "failed");
    assert!(error.contains("permanent"), "{error}");
    Ok(())
}

#[tokio::test]
#[ignore = "requires FORCE_SYNC_TEST_DATABASE_URL"]
async fn two_changes_to_one_record_both_settle() -> Result<(), ForceSyncError> {
    let harness = Harness::new().await?;
    Mock::given(method("PATCH"))
        .and(path(UPSERT_PATH))
        .respond_with(created())
        .mount(&harness.mock_server)
        .await;

    harness.insert_outbox_row("lsn-1", "Acme Corp").await?;
    harness
        .insert_outbox_row("lsn-2", "Acme Corporation")
        .await?;
    let runtime = harness.start(fast_config(3)).await?;
    let tasks = harness.wait_for_settled(2).await?;
    runtime.stop().await?;

    assert_eq!(
        tasks,
        vec![("done".to_owned(), None), ("done".to_owned(), None)]
    );
    assert_eq!(
        harness.linked_salesforce_id().await?.as_deref(),
        Some(SF_ID)
    );
    Ok(())
}

/// The durability property: a restart after the Salesforce call resumes at
/// finalize, and does not call Salesforce again.
///
/// A trigger makes every `sync_link` write fail, so finalize fails after the
/// Salesforce call succeeds. The runtime stops while finalize waits for its
/// retry. A new runtime then starts with the trigger removed.
#[tokio::test]
#[ignore = "requires FORCE_SYNC_TEST_DATABASE_URL"]
async fn restart_after_the_salesforce_call_does_not_repeat_it() -> Result<(), ForceSyncError> {
    let harness = Harness::new().await?;
    Mock::given(method("PATCH"))
        .and(path(UPSERT_PATH))
        .respond_with(created())
        .mount(&harness.mock_server)
        .await;
    harness
        .pool
        .get()
        .await?
        .batch_execute(
            "create function fail_link_writes() returns trigger language plpgsql as $$
             begin raise exception 'injected finalize failure'; end $$;
             create trigger fail_link_writes before insert or update on sync_link
             for each row execute function fail_link_writes();",
        )
        .await?;

    harness.insert_outbox_row("lsn-1", "Acme Corp").await?;
    let first = harness.start(fast_config(3)).await?;

    let deadline = Instant::now() + Duration::from_secs(60);
    while harness.salesforce_calls().await == 0 {
        assert!(Instant::now() < deadline, "Salesforce was never called");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    // Let finalize fail at least once, so the Salesforce result is recorded.
    tokio::time::sleep(Duration::from_secs(2)).await;
    first.stop().await?;
    assert_eq!(harness.tasks().await?[0].0, "leased");
    assert_eq!(harness.salesforce_calls().await, 1);

    harness
        .pool
        .get()
        .await?
        .batch_execute("drop trigger fail_link_writes on sync_link;")
        .await?;
    let second = harness.start(fast_config(3)).await?;
    let tasks = harness.wait_for_settled(1).await?;
    second.stop().await?;

    assert_eq!(tasks, vec![("done".to_owned(), None)]);
    assert_eq!(
        harness.linked_salesforce_id().await?.as_deref(),
        Some(SF_ID)
    );
    assert_eq!(
        harness.salesforce_calls().await,
        1,
        "recovery must resume at finalize, not repeat the Salesforce call"
    );
    Ok(())
}

/// A crash after the dispatch lease commits, but before the apply workflow
/// starts, leaves the task `leased` with no workflow. The next dispatch must
/// reclaim it once the lease expires.
#[tokio::test]
#[ignore = "requires FORCE_SYNC_TEST_DATABASE_URL"]
async fn task_stranded_by_a_dispatch_crash_is_recovered() -> Result<(), ForceSyncError> {
    let harness = Harness::new().await?;
    Mock::given(method("PATCH"))
        .and(path(UPSERT_PATH))
        .respond_with(created())
        .expect(1)
        .mount(&harness.mock_server)
        .await;

    harness.insert_outbox_row("lsn-1", "Acme Corp").await?;
    let engine = SyncEngine::builder(test_client(&harness.mock_server).await)
        .postgres(PgStore::new(harness.pool.clone()))
        .object(ObjectSync::new("Account").external_id("ExternalId__c"))
        .build()?;
    assert_eq!(engine.run_capture_postgres_once().await?, 1);
    harness
        .pool
        .get()
        .await?
        .execute(
            "update sync_task
             set status = 'leased', lease_owner = $1,
                 lease_until = now() - interval '1 minute'",
            &[&force_sync::durable::DEFAULT_LEASE_OWNER],
        )
        .await?;

    let runtime = harness.start(fast_config(3)).await?;
    let tasks = harness.wait_for_settled(1).await?;
    runtime.stop().await?;

    assert_eq!(tasks, vec![("done".to_owned(), None)]);
    assert_eq!(
        harness.linked_salesforce_id().await?.as_deref(),
        Some(SF_ID)
    );
    Ok(())
}

// -- Durable Salesforce CDC capture --------------------------------------

/// A CDC source with Salesforce replay semantics over an in-memory feed.
///
/// `Custom(id)` delivers the events after `id`. `Earliest` delivers all of
/// them. After the feed it idles, as a quiet channel does. It records each
/// preset it is asked for.
#[derive(Default)]
struct FeedSource {
    feed: Mutex<Vec<(u8, Value)>>,
    presets: Mutex<Vec<String>>,
}

impl FeedSource {
    fn publish(&self, replay: u8, external_id: &str, salesforce_id: &str) {
        self.feed
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push((
                replay,
                json!({
                    "ChangeEventHeader": {"entityName": "Account", "changeType": "UPDATE"},
                    "Id": salesforce_id,
                    "ExternalId__c": external_id,
                    "Name": format!("Acme {replay}"),
                }),
            ));
    }

    fn presets(&self) -> Vec<String> {
        self.presets
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

#[async_trait]
impl CdcSource for FeedSource {
    async fn subscribe(
        &self,
        _topic: &str,
        replay: ReplayPreset,
    ) -> Result<CdcEventStream, ForceSyncError> {
        let after = match &replay {
            ReplayPreset::Custom(id) => id.as_bytes().last().copied(),
            ReplayPreset::Latest | ReplayPreset::Earliest => None,
        };
        self.presets
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(match replay {
                ReplayPreset::Latest => "latest".to_owned(),
                ReplayPreset::Earliest => "earliest".to_owned(),
                ReplayPreset::Custom(id) => format!("custom:{:?}", id.as_bytes()),
            });
        let events: Vec<_> = self
            .feed
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .filter(|(replay, _)| after.is_none_or(|after| *replay > after))
            .map(|(replay, payload)| {
                Ok(PubSubEvent::Event(EventMessage {
                    payload: payload.clone(),
                    replay_id: ReplayId::from_bytes(vec![*replay]),
                    schema_id: "schema-1".to_owned(),
                    event_id: format!("evt-{replay}"),
                }))
            })
            .collect();
        Ok(stream::iter(events).chain(stream::pending()).boxed())
    }
}

/// Short CDC windows and a fast supervisor, so tests settle in seconds.
fn cdc_config() -> DurableSyncConfig {
    DurableSyncConfig {
        cdc_window: CdcWindow {
            max_duration: Duration::from_secs(2),
            max_events: 100,
        },
        cdc_supervise_interval: Duration::from_secs(1),
        cdc_error_backoff: Duration::from_secs(1),
        ..fast_config(3)
    }
}

const SF_ID_1: &str = "001000000000011AAA";
const SF_ID_2: &str = "001000000000012AAA";
const SF_ID_3: &str = "001000000000013AAA";

impl Harness {
    async fn salesforce_journal_rows(&self) -> Result<i64, ForceSyncError> {
        Ok(self
            .pool
            .get()
            .await?
            .query_one(
                "select count(*) from sync_journal where source = 'salesforce'",
                &[],
            )
            .await?
            .get(0))
    }

    async fn linked_ids(&self) -> Result<Vec<(String, Option<String>)>, ForceSyncError> {
        Ok(self
            .pool
            .get()
            .await?
            .query(
                "select external_id, salesforce_id from sync_link
                 where last_source = 'salesforce' order by external_id",
                &[],
            )
            .await?
            .iter()
            .map(|row| (row.get(0), row.get(1)))
            .collect())
    }
}

#[tokio::test]
#[ignore = "requires FORCE_SYNC_TEST_DATABASE_URL"]
async fn cdc_events_are_captured_and_projected_by_the_durable_runtime() -> Result<(), ForceSyncError>
{
    let harness = Harness::new().await?;
    let source = Arc::new(FeedSource::default());
    source.publish(1, "cdc-1", SF_ID_1);
    source.publish(2, "cdc-2", SF_ID_2);

    let runtime = harness.start_with_cdc(cdc_config(), source.clone()).await?;
    let tasks = harness.wait_for_settled(2).await?;
    // The supervisor fires every second. Let it fire several more times.
    tokio::time::sleep(Duration::from_secs(4)).await;
    let live_stream_runs: i64 = harness
        .pool
        .get()
        .await?
        .query_one(
            "select count(*) from harvest_workflow_executions
             where workflow_id = 'force-sync-cdc-salesforce:Account'
               and state not in ('COMPLETED', 'FAILED', 'CANCELLED', 'TERMINATED',
                                 'TIMED_OUT', 'CONTINUED_AS_NEW')",
            &[],
        )
        .await?
        .get(0);
    runtime.stop().await?;

    assert_eq!(
        live_stream_runs, 1,
        "the supervisor attaches, never duplicates"
    );
    assert_eq!(
        tasks,
        vec![("done".to_owned(), None), ("done".to_owned(), None)]
    );
    assert_eq!(
        harness.linked_ids().await?,
        vec![
            ("cdc-1".to_owned(), Some(SF_ID_1.to_owned())),
            ("cdc-2".to_owned(), Some(SF_ID_2.to_owned())),
        ]
    );
    assert_eq!(
        harness.salesforce_calls().await,
        0,
        "a Salesforce-origin change is projected locally, never written back"
    );
    assert_eq!(
        source.presets().first().map(String::as_str),
        Some("earliest")
    );
    Ok(())
}

/// A restart resumes the stream from its checkpoint: it neither loses an
/// event published while it was down nor journals one twice.
#[tokio::test]
#[ignore = "requires FORCE_SYNC_TEST_DATABASE_URL"]
async fn cdc_capture_resumes_from_the_checkpoint_after_a_restart() -> Result<(), ForceSyncError> {
    let harness = Harness::new().await?;
    let source = Arc::new(FeedSource::default());
    source.publish(1, "cdc-1", SF_ID_1);
    source.publish(2, "cdc-2", SF_ID_2);

    let first = harness.start_with_cdc(cdc_config(), source.clone()).await?;
    harness.wait_for_settled(2).await?;
    first.stop().await?;

    // Published while no runtime is up.
    source.publish(3, "cdc-3", SF_ID_3);
    let before_restart = source.presets().len();

    let second = harness.start_with_cdc(cdc_config(), source.clone()).await?;
    let tasks = harness.wait_for_settled(3).await?;
    second.stop().await?;

    assert!(
        tasks.iter().all(|(status, _)| status == "done"),
        "{tasks:?}"
    );
    assert_eq!(harness.salesforce_journal_rows().await?, 3, "no duplicates");
    assert_eq!(harness.linked_ids().await?.len(), 3);
    assert_eq!(
        source.presets().get(before_restart).map(String::as_str),
        Some("custom:[2]"),
        "the first subscription after the restart must resume after replay 2: {:?}",
        source.presets()
    );
    Ok(())
}

// -- Several harvest shards ---------------------------------------------

/// The second harvest shard. CI and local runs point it at its own database.
fn shard1_url() -> String {
    std::env::var("FORCE_SYNC_TEST_HARVEST_SHARD1_URL")
        .unwrap_or_else(|_| panic!("FORCE_SYNC_TEST_HARVEST_SHARD1_URL must be set"))
}

fn two_shard_router() -> force_sync::durable::ShardRouter {
    use force_sync::durable::{ShardId, ShardRouter};
    let shards = vec![ShardId::new(0), ShardId::new(1)];
    ShardRouter::new(shards.clone(), shards, ShardId::new(0))
}

async fn reset_and_migrate_shard1() -> Result<deadpool_postgres::Pool, ForceSyncError> {
    let mut config = deadpool_postgres::Config::new();
    config.url = Some(shard1_url());
    let pool = config
        .create_pool(
            Some(deadpool_postgres::Runtime::Tokio1),
            tokio_postgres::NoTls,
        )
        .unwrap_or_else(|error| panic!("shard 1 pool: {error}"));
    support::postgres::reset_schema(&pool).await?;
    migrate_harvest(&shard1_url()).await?;
    Ok(pool)
}

/// The workflow IDs of `workflow_name` that harvest stored on one shard.
async fn workflow_ids_on(
    pool: &deadpool_postgres::Pool,
    workflow_name: &str,
) -> Result<std::collections::BTreeSet<String>, ForceSyncError> {
    Ok(pool
        .get()
        .await?
        .query(
            "select distinct workflow_id from harvest_workflow_executions
             where workflow_name = $1",
            &[&workflow_name],
        )
        .await?
        .iter()
        .map(|row| row.get(0))
        .collect())
}

/// Inserts one outbox row for each `(record, cursor)` pair, on `ext-{record}`.
async fn insert_outbox_changes(
    pool: &deadpool_postgres::Pool,
    changes: &[(usize, usize)],
) -> Result<(), ForceSyncError> {
    for (record, cursor) in changes {
        pool.get()
            .await?
            .execute(
                "insert into force_sync_outbox
                    (tenant, object_name, external_id, source_cursor, op, tombstone, payload)
                 values ('tenant', 'Account', $1, $2, 'upsert', false, $3::jsonb)",
                &[
                    &format!("ext-{record}"),
                    &format!("lsn-{cursor}"),
                    &json!({ "Name": format!("Acme {record} v{cursor}") }),
                ],
            )
            .await?;
    }
    Ok(())
}

/// Where the router places each apply task: its workflow ID, its record,
/// and its shard (0 or 1).
///
/// An apply workflow is placed by its record key, the JSON array
/// `[tenant, object, external_id]`, so all tasks for one record share a
/// shard. That is what lets harvest's per-shard concurrency key serialize
/// them.
async fn expected_apply_placement(
    pool: &deadpool_postgres::Pool,
    router: &force_sync::durable::ShardRouter,
) -> Result<Vec<(String, String, usize)>, ForceSyncError> {
    use force_sync::durable::{APPLY_WORKFLOW, ShardId};
    let rows = pool
        .get()
        .await?
        .query(
            "select t.task_id, j.tenant, j.object_name, j.external_id
             from sync_task t
             join sync_journal j on (t.payload->>'journal_id')::bigint = j.journal_id
             order by t.task_id",
            &[],
        )
        .await?;
    Ok(rows
        .iter()
        .map(|row| {
            let tenant: String = row.get(1);
            let object_name: String = row.get(2);
            let external_id: String = row.get(3);
            let record = json!([tenant, object_name, external_id]).to_string();
            let shard = router.pick_for_new_workflow(APPLY_WORKFLOW, &record);
            (
                format!("force-sync-apply-{}", row.get::<_, i64>(0)),
                external_id,
                usize::from(shard != ShardId::new(0)),
            )
        })
        .collect())
}

/// Two harvest shards: each apply task and each CDC stream runs on the
/// shard that the router picks for its workflow ID, and only there.
#[tokio::test]
#[ignore = "requires FORCE_SYNC_TEST_DATABASE_URL and FORCE_SYNC_TEST_HARVEST_SHARD1_URL"]
async fn workflows_run_on_the_shard_the_router_picks() -> Result<(), ForceSyncError> {
    use force_sync::durable::{APPLY_WORKFLOW, CDC_STREAM_WORKFLOW, ShardId, harvest_sharded_pool};

    let harness = Harness::new().await?;
    let shard1 = reset_and_migrate_shard1().await?;
    Mock::given(method("PATCH"))
        .and(path_regex(
            r"^/services/data/v68\.0/sobjects/Account/ExternalId__c/ext-\d+$",
        ))
        .respond_with(created())
        .mount(&harness.mock_server)
        .await;
    // Eight records, and a second change to records 1 and 2.
    let mut changes: Vec<(usize, usize)> = (1..=8).map(|n| (n, n)).collect();
    changes.extend([(1, 9), (2, 10)]);
    insert_outbox_changes(&harness.pool, &changes).await?;
    let source = Arc::new(FeedSource::default());
    source.publish(1, "cdc-1", SF_ID_1);

    // Postgres owns `Name`, so the second change to a record merges.
    let engine = SyncEngine::builder(test_client(&harness.mock_server).await)
        .postgres(PgStore::new(harness.pool.clone()))
        .object(
            ObjectSync::new("Account")
                .external_id("ExternalId__c")
                .field_owner("Name", force_sync::Owner::Postgres),
        )
        .build()?;
    let pool = harvest_sharded_pool(
        [
            (ShardId::new(0), database_url()),
            (ShardId::new(1), shard1_url()),
        ],
        ShardId::new(0),
        8,
    )?;
    let runtime = DurableSync::new_sharded(engine, pool, two_shard_router())?
        .with_config(cdc_config())?
        .with_cdc(
            source.clone(),
            [CdcStream::new("tenant", "Account").starting_at(CdcStart::Earliest)],
        )?
        .start()?;
    let tasks = harness.wait_for_settled(11).await?;
    runtime.stop().await?;
    assert!(
        tasks.iter().all(|(status, _)| status == "done"),
        "{tasks:?}"
    );

    // Predict each workflow's shard from the router, then compare.
    let router = two_shard_router();
    let placement = expected_apply_placement(&harness.pool, &router).await?;
    for record in ["ext-1", "ext-2"] {
        let record_shards: std::collections::BTreeSet<usize> = placement
            .iter()
            .filter(|(_, external_id, _)| external_id == record)
            .map(|(_, _, shard)| *shard)
            .collect();
        assert_eq!(
            record_shards.len(),
            1,
            "both tasks of {record} must share a shard"
        );
    }
    let mut expected_apply = [
        std::collections::BTreeSet::new(),
        std::collections::BTreeSet::new(),
    ];
    for (workflow_id, _, shard) in placement {
        expected_apply[shard].insert(workflow_id);
    }
    assert!(
        expected_apply.iter().all(|ids| !ids.is_empty()),
        "the test needs work on both shards: {expected_apply:?}"
    );
    assert_eq!(
        workflow_ids_on(&harness.pool, APPLY_WORKFLOW).await?,
        expected_apply[0],
        "shard 0 apply workflows"
    );
    assert_eq!(
        workflow_ids_on(&shard1, APPLY_WORKFLOW).await?,
        expected_apply[1],
        "shard 1 apply workflows"
    );

    let stream_id = "force-sync-cdc-salesforce:Account".to_owned();
    let stream_shard = router.pick_for_new_workflow(CDC_STREAM_WORKFLOW, &stream_id);
    let (home, other) = if stream_shard == ShardId::new(0) {
        (&harness.pool, &shard1)
    } else {
        (&shard1, &harness.pool)
    };
    assert_eq!(
        workflow_ids_on(home, CDC_STREAM_WORKFLOW).await?,
        std::collections::BTreeSet::from([stream_id]),
        "the stream workflow lives on its router shard"
    );
    assert!(
        workflow_ids_on(other, CDC_STREAM_WORKFLOW)
            .await?
            .is_empty(),
        "the supervisor must never start the stream on another shard"
    );
    Ok(())
}
