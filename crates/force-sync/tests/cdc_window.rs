//! Bounded CDC capture windows: resume, limits, and poison-event quarantine.

// The stream item type is fixed by force-pubsub, and its error is large.
#![allow(clippy::result_large_err)]

mod support;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use force_pubsub::{EventMessage, PubSubError, PubSubEvent, ReplayId, ReplayPreset};
use futures::{StreamExt, stream};
use serde_json::{Value, json};

use force_sync::{
    CdcEventStream, CdcSource, CdcStart, CdcStream, CdcWindow, CdcWindowEnd, ForceSyncError,
    ObjectSync, PgStore, capture_cdc_window,
};

type Item = Result<PubSubEvent<Value>, PubSubError>;

#[derive(Debug, Clone)]
struct StaticAuth;

#[async_trait]
impl force::auth::Authenticator for StaticAuth {
    async fn authenticate(&self) -> force::error::Result<force::auth::AccessToken> {
        Ok(force::auth::AccessToken::from_response(
            force::auth::TokenResponse {
                access_token: secrecy::SecretString::new("token".to_owned().into()),
                instance_url: "https://unused.invalid".to_owned(),
                token_type: "Bearer".to_owned(),
                issued_at: "1704067200000".to_owned(),
                signature: "sig".to_owned(),
                expires_in: Some(7200),
                refresh_token: None,
            },
        ))
    }

    async fn refresh(&self) -> force::error::Result<force::auth::AccessToken> {
        self.authenticate().await
    }
}

/// A CDC source that replays a scripted list of items, then idles or ends.
///
/// It records each replay preset that it is asked to start from.
struct ScriptedSource {
    script: Mutex<Vec<Vec<Item>>>,
    end_after_script: bool,
    presets: Mutex<Vec<String>>,
}

impl ScriptedSource {
    fn new(windows: Vec<Vec<Item>>, end_after_script: bool) -> Arc<Self> {
        Arc::new(Self {
            script: Mutex::new(windows.into_iter().rev().collect()),
            end_after_script,
            presets: Mutex::new(Vec::new()),
        })
    }

    fn presets(&self) -> Vec<String> {
        self.presets
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

fn describe(preset: &ReplayPreset) -> String {
    match preset {
        ReplayPreset::Latest => "latest".to_owned(),
        ReplayPreset::Earliest => "earliest".to_owned(),
        ReplayPreset::Custom(id) => format!("custom:{:?}", id.as_bytes()),
    }
}

#[async_trait]
impl CdcSource for ScriptedSource {
    async fn subscribe(
        &self,
        _topic: &str,
        replay: ReplayPreset,
    ) -> Result<CdcEventStream, ForceSyncError> {
        self.presets
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(describe(&replay));
        let items = self
            .script
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .pop()
            .unwrap_or_default();
        let head = stream::iter(items);
        Ok(if self.end_after_script {
            head.boxed()
        } else {
            head.chain(stream::pending()).boxed()
        })
    }
}

fn event(replay: u8, external_id: Option<&str>) -> PubSubEvent<Value> {
    let mut payload = json!({
        "ChangeEventHeader": {"entityName": "Account", "changeType": "UPDATE"},
        "Name": format!("Acme {replay}"),
    });
    if let Some(external_id) = external_id {
        payload["ExternalId__c"] = json!(external_id);
    }
    PubSubEvent::Event(EventMessage {
        payload,
        replay_id: ReplayId::from_bytes(vec![replay]),
        schema_id: "schema-1".to_owned(),
        event_id: format!("evt-{replay}"),
    })
}

fn account_stream() -> CdcStream {
    CdcStream::new("tenant", "Account").starting_at(CdcStart::Earliest)
}

fn account() -> ObjectSync {
    ObjectSync::new("Account").external_id("ExternalId__c")
}

const fn short_window(max_events: usize) -> CdcWindow {
    CdcWindow {
        max_duration: Duration::from_millis(300),
        max_events,
    }
}

async fn fresh_store() -> Result<(deadpool_postgres::Pool, PgStore), ForceSyncError> {
    let pool = support::postgres::test_pool();
    support::postgres::reset_schema(&pool).await?;
    force_sync::migrate(&pool).await?;
    Ok((pool.clone(), PgStore::new(pool)))
}

async fn count(pool: &deadpool_postgres::Pool, sql: &str) -> Result<i64, ForceSyncError> {
    Ok(pool.get().await?.query_one(sql, &[]).await?.get(0))
}

async fn checkpoint(pool: &deadpool_postgres::Pool) -> Result<Option<i64>, ForceSyncError> {
    Ok(pool
        .get()
        .await?
        .query_opt(
            "select cursor_position from sync_checkpoint where stream_name = 'salesforce:Account'",
            &[],
        )
        .await?
        .map(|row| row.get(0)))
}

#[test]
fn cdc_stream_derives_channel_and_checkpoint_names() {
    let standard = CdcStream::new("tenant", "Account");
    assert_eq!(standard.topic(), "/data/AccountChangeEvent");
    assert_eq!(standard.name(), "salesforce:Account");
    assert_eq!(standard.start(), CdcStart::Latest);

    let custom = CdcStream::new("tenant", "Invoice__c");
    assert_eq!(custom.topic(), "/data/Invoice__ChangeEvent");

    let renamed = CdcStream::new("tenant", "Account")
        .with_topic("/data/AccountsOnly__chn")
        .with_name("salesforce:Account:custom");
    assert_eq!(renamed.topic(), "/data/AccountsOnly__chn");
    assert_eq!(renamed.name(), "salesforce:Account:custom");
}

#[tokio::test]
#[ignore = "requires FORCE_SYNC_TEST_DATABASE_URL"]
async fn window_captures_then_next_window_resumes_from_the_checkpoint() -> Result<(), ForceSyncError>
{
    let (pool, store) = fresh_store().await?;
    let source = ScriptedSource::new(
        vec![
            vec![Ok(event(1, Some("ext-1"))), Ok(event(2, Some("ext-2")))],
            vec![Ok(event(3, Some("ext-3")))],
        ],
        false,
    );

    let first = capture_cdc_window(
        &store,
        source.as_ref(),
        &account_stream(),
        &account(),
        short_window(100),
    )
    .await?;
    assert_eq!(first.captured, 2);
    assert_eq!(first.end, CdcWindowEnd::Deadline);
    assert_eq!(checkpoint(&pool).await?, Some(2));

    let second = capture_cdc_window(
        &store,
        source.as_ref(),
        &account_stream(),
        &account(),
        short_window(100),
    )
    .await?;
    assert_eq!(second.captured, 1);
    assert_eq!(checkpoint(&pool).await?, Some(3));

    assert_eq!(source.presets(), vec!["earliest", "custom:[2]"]);
    assert_eq!(
        count(
            &pool,
            "select count(*) from sync_task where task_kind = 'apply'"
        )
        .await?,
        3
    );
    Ok(())
}

#[tokio::test]
#[ignore = "requires FORCE_SYNC_TEST_DATABASE_URL"]
async fn window_stops_at_max_events() -> Result<(), ForceSyncError> {
    let (pool, store) = fresh_store().await?;
    let source = ScriptedSource::new(
        vec![
            (1..=5)
                .map(|n| Ok(event(n, Some(&format!("ext-{n}")))))
                .collect(),
        ],
        false,
    );

    let report = capture_cdc_window(
        &store,
        source.as_ref(),
        &account_stream(),
        &account(),
        short_window(3),
    )
    .await?;

    assert_eq!(report.captured, 3);
    assert_eq!(report.end, CdcWindowEnd::MaxEvents);
    assert_eq!(checkpoint(&pool).await?, Some(3));
    Ok(())
}

#[tokio::test]
#[ignore = "requires FORCE_SYNC_TEST_DATABASE_URL"]
async fn event_without_external_id_is_quarantined_and_does_not_wedge_the_stream()
-> Result<(), ForceSyncError> {
    let (pool, store) = fresh_store().await?;
    let source = ScriptedSource::new(
        vec![vec![Ok(event(1, None)), Ok(event(2, Some("ext-2")))]],
        true,
    );

    let report = capture_cdc_window(
        &store,
        source.as_ref(),
        &account_stream(),
        &account(),
        short_window(100),
    )
    .await?;

    assert_eq!(report.quarantined, 1);
    assert_eq!(report.captured, 1);
    assert_eq!(report.end, CdcWindowEnd::StreamEnded);
    assert_eq!(checkpoint(&pool).await?, Some(2));

    let row = pool
        .get()
        .await?
        .query_one(
            "select tenant, object_name, error_message, payload->>'Name'
             from sync_dead_letter",
            &[],
        )
        .await?;
    assert_eq!(row.get::<_, Option<String>>(0).as_deref(), Some("tenant"));
    assert_eq!(row.get::<_, Option<String>>(1).as_deref(), Some("Account"));
    assert!(row.get::<_, String>(2).contains("external_id"));
    assert_eq!(row.get::<_, Option<String>>(3).as_deref(), Some("Acme 1"));
    Ok(())
}

#[tokio::test]
#[ignore = "requires FORCE_SYNC_TEST_DATABASE_URL"]
async fn redelivered_events_count_as_duplicates() -> Result<(), ForceSyncError> {
    let (pool, store) = fresh_store().await?;
    let source = ScriptedSource::new(
        vec![vec![
            Ok(event(1, Some("ext-1"))),
            Ok(event(1, Some("ext-1"))),
            Ok(event(2, Some("ext-2"))),
        ]],
        true,
    );

    let report = capture_cdc_window(
        &store,
        source.as_ref(),
        &account_stream(),
        &account(),
        short_window(100),
    )
    .await?;

    assert_eq!(report.captured, 2);
    assert_eq!(report.duplicates, 1);
    assert_eq!(count(&pool, "select count(*) from sync_journal").await?, 2);
    Ok(())
}

#[tokio::test]
#[ignore = "requires FORCE_SYNC_TEST_DATABASE_URL"]
async fn stream_error_fails_the_window_but_keeps_committed_events() -> Result<(), ForceSyncError> {
    let (pool, store) = fresh_store().await?;
    let source = ScriptedSource::new(
        vec![vec![
            Ok(event(1, Some("ext-1"))),
            Err(PubSubError::Config("stream dropped".to_owned())),
        ]],
        false,
    );

    let result = capture_cdc_window(
        &store,
        source.as_ref(),
        &account_stream(),
        &account(),
        short_window(100),
    )
    .await;

    let Err(error) = result else {
        panic!("a stream error must fail the window");
    };
    assert!(error.to_string().contains("stream dropped"), "{error}");
    assert_eq!(checkpoint(&pool).await?, Some(1));
    Ok(())
}

#[tokio::test]
#[ignore = "requires FORCE_SYNC_TEST_DATABASE_URL"]
async fn engine_window_requires_a_configured_object() -> Result<(), ForceSyncError> {
    let (_pool, store) = fresh_store().await?;
    let client = force::client::builder()
        .authenticate(StaticAuth)
        .build()
        .await
        .unwrap_or_else(|error| panic!("client: {error}"));
    let engine = force_sync::SyncEngine::builder(client)
        .postgres(store)
        .object(ObjectSync::new("Contact").external_id("ExternalId__c"))
        .build()?;
    let source = ScriptedSource::new(vec![vec![Ok(event(1, Some("ext-1")))]], true);

    let result = engine
        .run_cdc_window(source.as_ref(), &account_stream(), short_window(10))
        .await;

    assert!(matches!(
        result,
        Err(ForceSyncError::MissingConfiguration {
            field: "object sync"
        })
    ));
    assert!(
        source.presets().is_empty(),
        "no subscription without config"
    );
    Ok(())
}

/// A source that stays quiet, then sends one keepalive after `delay`.
struct QuietSource {
    delay: Duration,
    latest: u8,
    presets: Mutex<Vec<String>>,
}

#[async_trait]
impl CdcSource for QuietSource {
    async fn subscribe(
        &self,
        _topic: &str,
        replay: ReplayPreset,
    ) -> Result<CdcEventStream, ForceSyncError> {
        self.presets
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(describe(&replay));
        let delay = self.delay;
        let latest = self.latest;
        let keepalive = stream::once(async move {
            tokio::time::sleep(delay).await;
            Ok(PubSubEvent::KeepAlive {
                latest_replay_id: Some(ReplayId::from_bytes(vec![latest])),
            })
        });
        Ok(keepalive.chain(stream::pending()).boxed())
    }
}

#[tokio::test]
#[ignore = "requires FORCE_SYNC_TEST_DATABASE_URL"]
async fn keepalive_checkpoints_a_quiet_stream_so_the_next_window_misses_nothing()
-> Result<(), ForceSyncError> {
    let (pool, store) = fresh_store().await?;
    let source = QuietSource {
        delay: Duration::ZERO,
        latest: 7,
        presets: Mutex::new(Vec::new()),
    };
    let quiet = CdcStream::new("tenant", "Account");

    let first = capture_cdc_window(&store, &source, &quiet, &account(), short_window(10)).await?;
    assert_eq!(first.captured, 0);
    assert_eq!(checkpoint(&pool).await?, Some(7));

    capture_cdc_window(&store, &source, &quiet, &account(), short_window(10)).await?;
    let presets = source
        .presets
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    assert_eq!(
        presets,
        vec!["latest", "custom:[7]"],
        "a second Latest subscription would skip the events between windows"
    );
    Ok(())
}

#[tokio::test]
#[ignore = "requires FORCE_SYNC_TEST_DATABASE_URL"]
async fn fresh_latest_stream_holds_its_first_window_until_it_can_resume()
-> Result<(), ForceSyncError> {
    let (pool, store) = fresh_store().await?;
    let source = QuietSource {
        delay: Duration::from_millis(900),
        latest: 5,
        presets: Mutex::new(Vec::new()),
    };
    let quiet = CdcStream::new("tenant", "Account");

    let started = std::time::Instant::now();
    let report = capture_cdc_window(&store, &source, &quiet, &account(), short_window(10)).await?;

    assert!(
        started.elapsed() >= Duration::from_millis(900),
        "the window ended at {:?}, before it had a position to resume from",
        started.elapsed()
    );
    assert_eq!(report.end, CdcWindowEnd::Deadline);
    assert_eq!(checkpoint(&pool).await?, Some(5));
    Ok(())
}
