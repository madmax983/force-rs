//! Live Salesforce Change Data Capture contract tests for force-sync.
//!
//! These tests run real change events from a Salesforce org through
//! force-sync capture. They create, update and delete one record whose key
//! starts with [`LIVE_TEST_PREFIX`], and they always try to delete it again.
//!
//! They are ignored by default. Each test skips, and passes, unless all of
//! these are set:
//!
//! - one Salesforce auth configuration, with the same variables as the other
//!   live binaries (ADR-033): `SF_JWT_*`, `SF_CLIENT_*` + `SF_TOKEN_URL`,
//!   `SF_UP_*`, or `SF_ACCESS_TOKEN` + `SF_INSTANCE_URL`
//! - `SF_SYNC_CDC=1`, which opts in to this tier
//! - `FORCE_SYNC_TEST_DATABASE_URL`, a Postgres database that the tests
//!   reset
//!
//! Change Data Capture must be on for the object in the org (Setup, Change
//! Data Capture). Optional variables:
//!
//! - `SF_SYNC_CDC_OBJECT` (default `Account`)
//! - `SF_SYNC_CDC_EXTERNAL_ID_FIELD` (default `Name`), a text field that the
//!   test writes the record key to
//! - `SF_SYNC_CDC_UPDATE_FIELD` (default `Description`), a text field that
//!   the sparse update changes
//! - `SF_SYNC_CDC_RECORD_JSON`, a JSON object of extra fields for the create,
//!   such as the required `LastName` of a `Contact`
//! - `SF_SYNC_CDC_TOPIC`, a custom channel to subscribe to instead of the
//!   standard `/data/{Object}ChangeEvent` channel
//! - `SF_PUBSUB_ENDPOINT` (default Salesforce's global Pub/Sub endpoint)
//! - `SF_API_VERSION` (default `v62.0`)
//!
//! Each delivered event counts against the org's daily CDC delivery
//! allocation. A test subscribes from `Latest` for about a minute, so it
//! receives only the events of that minute. In a busy shared org, set
//! `SF_SYNC_CDC_TOPIC` to a custom channel with a filter on the key prefix.

// The CDC stream item type is fixed by force-pubsub, and its error is large.
#![allow(clippy::result_large_err)]

mod support;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context as _, Result, bail, ensure};
use async_trait::async_trait;
use force::api::RestOperation as _;
use force::auth::{AccessToken, Authenticator, TokenResponse};
use force::client::{ForceClient, builder};
use force::config::ClientConfig;
use force::types::SalesforceId;
use force_pubsub::{PubSubConfig, PubSubHandler, ReplayPreset};
use serde_json::{Map, Value, json};

use force_sync::{
    CdcEventStream, CdcSource, CdcStream, CdcWindow, ForceSyncError, ObjectSync, PgStore,
    SyncEngine,
};

/// The prefix of every record key that these tests write.
const LIVE_TEST_PREFIX: &str = "force-rs-live-test";
const DEFAULT_PUBSUB_ENDPOINT: &str = "https://api.pubsub.salesforce.com:7443";
const TENANT: &str = "live";

/// How long a test waits for one change to reach the journal.
const CHANGE_TIMEOUT: Duration = Duration::from_secs(60);
/// How long a test waits after the subscription opens, before it writes.
const SUBSCRIBE_SETTLE: Duration = Duration::from_secs(10);
/// How many keyed writes a test tries before the subscription must be live.
const KEYED_ATTEMPTS: usize = 3;

// ─── Auth (same variables and order as ADR-033) ────────────────────────────

#[derive(Debug, Clone)]
enum LiveAuth {
    Jwt(force::auth::JwtBearerFlow),
    ClientCredentials(force::auth::ClientCredentials),
    UsernamePassword(force::auth::UsernamePassword),
    Token(EnvAuthenticator),
}

#[async_trait]
impl Authenticator for LiveAuth {
    async fn authenticate(&self) -> force::error::Result<AccessToken> {
        match self {
            Self::Jwt(flow) => flow.authenticate().await,
            Self::ClientCredentials(flow) => flow.authenticate().await,
            Self::UsernamePassword(flow) => flow.authenticate().await,
            Self::Token(env) => env.authenticate().await,
        }
    }

    async fn refresh(&self) -> force::error::Result<AccessToken> {
        match self {
            Self::Jwt(flow) => flow.refresh().await,
            Self::ClientCredentials(flow) => flow.refresh().await,
            Self::UsernamePassword(flow) => flow.refresh().await,
            Self::Token(env) => env.refresh().await,
        }
    }
}

#[derive(Debug, Clone)]
struct EnvAuthenticator {
    access_token: String,
    instance_url: String,
}

#[async_trait]
impl Authenticator for EnvAuthenticator {
    async fn authenticate(&self) -> force::error::Result<AccessToken> {
        Ok(AccessToken::from_response(TokenResponse {
            access_token: secrecy::SecretString::new(self.access_token.clone().into()),
            instance_url: self.instance_url.clone(),
            token_type: "Bearer".to_owned(),
            issued_at: chrono::Utc::now().timestamp_millis().to_string(),
            expires_in: Some(7_200),
            refresh_token: None,
            signature: "live-test".to_owned(),
        }))
    }

    async fn refresh(&self) -> force::error::Result<AccessToken> {
        self.authenticate().await
    }
}

fn env_string(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

/// Accepts a bare host, a base URL or a token endpoint.
fn token_url(raw: &str) -> String {
    let trimmed = raw.trim().trim_end_matches('/');
    let base = if trimmed.contains("://") {
        trimmed.to_owned()
    } else {
        format!("https://{trimmed}")
    };
    if base.ends_with("/services/oauth2/token") {
        base
    } else {
        format!("{base}/services/oauth2/token")
    }
}

fn try_jwt_auth() -> Option<LiveAuth> {
    let client_id = env_string("SF_JWT_CLIENT_ID")?;
    let username = env_string("SF_JWT_USERNAME")?;
    let key_path = env_string("SF_JWT_PRIVATE_KEY_PATH")?;
    let private_key = std::fs::read_to_string(&key_path).ok()?;
    let token_url = token_url(
        &env_string("SF_JWT_LOGIN_URL").unwrap_or_else(|| "https://login.salesforce.com".into()),
    );
    let audience = token_url
        .trim_end_matches("/services/oauth2/token")
        .to_owned();
    force::auth::JwtBearerFlow::new(client_id, username, private_key, audience, token_url)
        .ok()
        .map(LiveAuth::Jwt)
}

fn try_client_credentials_auth() -> Option<LiveAuth> {
    Some(LiveAuth::ClientCredentials(
        force::auth::ClientCredentials::new(
            env_string("SF_CLIENT_ID")?,
            env_string("SF_CLIENT_SECRET")?,
            token_url(&env_string("SF_TOKEN_URL")?),
        ),
    ))
}

fn try_username_password_auth() -> Option<LiveAuth> {
    Some(LiveAuth::UsernamePassword(
        force::auth::UsernamePassword::new(
            env_string("SF_UP_CLIENT_ID")?,
            env_string("SF_UP_CLIENT_SECRET")?,
            env_string("SF_UP_USERNAME")?,
            env_string("SF_UP_PASSWORD")?,
            env_string("SF_UP_SECURITY_TOKEN").unwrap_or_default(),
            token_url(
                &env_string("SF_UP_TOKEN_URL")
                    .unwrap_or_else(|| "https://login.salesforce.com".into()),
            ),
        ),
    ))
}

fn try_token_auth() -> Option<LiveAuth> {
    Some(LiveAuth::Token(EnvAuthenticator {
        access_token: env_string("SF_ACCESS_TOKEN")?,
        instance_url: env_string("SF_INSTANCE_URL")?,
    }))
}

fn load_live_auth() -> Option<LiveAuth> {
    try_jwt_auth()
        .or_else(try_client_credentials_auth)
        .or_else(try_username_password_auth)
        .or_else(try_token_auth)
}

// ─── Tier config ───────────────────────────────────────────────────────────

struct CdcTier {
    auth: LiveAuth,
    api_version: String,
    pubsub_endpoint: String,
    object_name: String,
    external_id_field: String,
    update_field: String,
    record_fields: Map<String, Value>,
    topic: Option<String>,
}

/// Loads the tier, or prints why the test skips.
fn load_cdc_tier(test_name: &str) -> Result<Option<CdcTier>> {
    let skip = |reason: &str| {
        eprintln!("SKIP {test_name}: {reason}");
        Ok(None)
    };
    if env_string("SF_SYNC_CDC").as_deref() != Some("1") {
        return skip("SF_SYNC_CDC is not 1");
    }
    if env_string("FORCE_SYNC_TEST_DATABASE_URL").is_none() {
        return skip("FORCE_SYNC_TEST_DATABASE_URL is not set");
    }
    let Some(auth) = load_live_auth() else {
        return skip("no Salesforce credentials (SF_JWT_*, SF_CLIENT_*, SF_UP_*, SF_ACCESS_TOKEN)");
    };
    let record_fields = match env_string("SF_SYNC_CDC_RECORD_JSON") {
        Some(raw) => match serde_json::from_str(&raw)
            .context("SF_SYNC_CDC_RECORD_JSON must be a JSON object")?
        {
            Value::Object(fields) => fields,
            _ => bail!("SF_SYNC_CDC_RECORD_JSON must be a JSON object"),
        },
        None => Map::new(),
    };

    Ok(Some(CdcTier {
        auth,
        api_version: env_string("SF_API_VERSION").unwrap_or_else(|| "v62.0".to_owned()),
        pubsub_endpoint: env_string("SF_PUBSUB_ENDPOINT")
            .unwrap_or_else(|| DEFAULT_PUBSUB_ENDPOINT.to_owned()),
        object_name: env_string("SF_SYNC_CDC_OBJECT").unwrap_or_else(|| "Account".to_owned()),
        external_id_field: env_string("SF_SYNC_CDC_EXTERNAL_ID_FIELD")
            .unwrap_or_else(|| "Name".to_owned()),
        update_field: env_string("SF_SYNC_CDC_UPDATE_FIELD")
            .unwrap_or_else(|| "Description".to_owned()),
        record_fields,
        topic: env_string("SF_SYNC_CDC_TOPIC"),
    }))
}

impl CdcTier {
    async fn client(&self) -> Result<ForceClient<LiveAuth>> {
        Ok(builder()
            .config(ClientConfig {
                api_version: self.api_version.clone(),
                ..ClientConfig::default()
            })
            .authenticate(self.auth.clone())
            .build()
            .await?)
    }

    fn object(&self) -> ObjectSync {
        ObjectSync::new(&self.object_name).external_id(&self.external_id_field)
    }

    fn stream(&self) -> CdcStream {
        let stream = CdcStream::new(TENANT, &self.object_name);
        match &self.topic {
            Some(topic) => stream.with_topic(topic),
            None => stream,
        }
    }

    async fn source(&self, client: &ForceClient<LiveAuth>) -> Result<Arc<OpenedSource>> {
        let handler = PubSubHandler::connect(
            client.session(),
            PubSubConfig {
                endpoint: self.pubsub_endpoint.clone(),
                ..PubSubConfig::default()
            },
        )
        .await?;
        Ok(Arc::new(OpenedSource {
            inner: handler,
            opened: AtomicUsize::new(0),
        }))
    }
}

/// A Pub/Sub source that counts the subscriptions it opens.
///
/// A test writes to Salesforce only after a subscription is open, so the
/// `Latest` subscription cannot miss the change.
struct OpenedSource {
    inner: PubSubHandler<LiveAuth>,
    opened: AtomicUsize,
}

#[async_trait]
impl CdcSource for OpenedSource {
    async fn subscribe(
        &self,
        topic: &str,
        replay: ReplayPreset,
    ) -> Result<CdcEventStream, ForceSyncError> {
        let stream = CdcSource::subscribe(&self.inner, topic, replay).await?;
        self.opened.fetch_add(1, Ordering::SeqCst);
        Ok(stream)
    }
}

impl OpenedSource {
    async fn wait_until_open(&self) -> Result<()> {
        let deadline = Instant::now() + CHANGE_TIMEOUT;
        while self.opened.load(Ordering::SeqCst) == 0 {
            ensure!(Instant::now() < deadline, "no CDC subscription opened");
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        tokio::time::sleep(SUBSCRIBE_SETTLE).await;
        Ok(())
    }
}

// ─── The test record ───────────────────────────────────────────────────────

fn unique_key(suffix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    format!("{LIVE_TEST_PREFIX}-cdc-{nanos}-{suffix}")
}

/// One Salesforce record that the test owns and deletes.
struct LiveRecord<'a> {
    client: &'a ForceClient<LiveAuth>,
    tier: &'a CdcTier,
    id: Option<SalesforceId>,
    deleted: bool,
}

impl<'a> LiveRecord<'a> {
    const fn new(client: &'a ForceClient<LiveAuth>, tier: &'a CdcTier) -> Self {
        Self {
            client,
            tier,
            id: None,
            deleted: false,
        }
    }

    fn id(&self) -> Result<&SalesforceId> {
        self.id.as_ref().context("the record is not created yet")
    }

    /// Creates the record with `key`, or renames it to `key`.
    async fn write_key(&mut self, key: &str) -> Result<()> {
        let field = self.tier.external_id_field.clone();
        if let Some(id) = &self.id {
            self.client
                .rest()
                .update(&self.tier.object_name, id, &json!({ field: key }))
                .await?;
            return Ok(());
        }
        let mut body = self.tier.record_fields.clone();
        body.insert(field, json!(key));
        let created = self
            .client
            .rest()
            .create(&self.tier.object_name, &Value::Object(body))
            .await?;
        ensure!(created.success, "create failed: {:?}", created.errors);
        self.id = Some(created.id.context("a successful create returns an ID")?);
        Ok(())
    }

    /// Changes one field that is not the key.
    async fn sparse_update(&self) -> Result<String> {
        let value = format!("{LIVE_TEST_PREFIX} sparse update");
        let field = self.tier.update_field.clone();
        self.client
            .rest()
            .update(&self.tier.object_name, self.id()?, &json!({ field: value }))
            .await?;
        Ok(value)
    }

    async fn delete(&mut self) -> Result<()> {
        self.client
            .rest()
            .delete(&self.tier.object_name, self.id()?)
            .await?;
        self.deleted = true;
        Ok(())
    }

    /// Deletes the record if the test did not, and reports any failure.
    async fn cleanup(&mut self) {
        if self.id.is_some() && !self.deleted {
            if let Err(error) = self.delete().await {
                eprintln!("cleanup: could not delete the live test record: {error}");
            }
        }
    }
}

// ─── Journal checks ────────────────────────────────────────────────────────

/// The journal rows of one record key, oldest first, as `(operation, payload)`.
async fn journal_rows(
    pool: &deadpool_postgres::Pool,
    tier: &CdcTier,
    key: &str,
) -> Result<Vec<(String, Value)>> {
    let rows = pool
        .get()
        .await?
        .query(
            "select operation, payload::text from sync_journal
             where tenant = $1 and object_name = $2 and external_id = $3
             order by journal_id",
            &[&TENANT, &tier.object_name, &key],
        )
        .await?;
    rows.iter()
        .map(|row| {
            let payload: String = row.get(1);
            Ok((row.get(0), serde_json::from_str(&payload)?))
        })
        .collect()
}

/// Waits until the record key has at least `count` journal rows.
async fn wait_for_rows(
    pool: &deadpool_postgres::Pool,
    tier: &CdcTier,
    key: &str,
    count: usize,
) -> Result<Option<Vec<(String, Value)>>> {
    let deadline = Instant::now() + CHANGE_TIMEOUT;
    loop {
        let rows = journal_rows(pool, tier, key).await?;
        if rows.len() >= count {
            return Ok(Some(rows));
        }
        if Instant::now() >= deadline {
            return Ok(None);
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

/// Writes a keyed change until one reaches the journal, and returns its key.
///
/// A `Latest` subscription can take a moment to go live after it opens. A
/// change made before that is not delivered. So the test renames the record
/// to a new key and waits again, up to [`KEYED_ATTEMPTS`] times. A keyed
/// change carries the key and the record ID, so capture maps the record.
async fn write_until_captured(
    pool: &deadpool_postgres::Pool,
    tier: &CdcTier,
    record: &mut LiveRecord<'_>,
) -> Result<String> {
    for attempt in 0..KEYED_ATTEMPTS {
        let key = unique_key(&attempt.to_string());
        record.write_key(&key).await?;
        if wait_for_rows(pool, tier, &key, 1).await?.is_some() {
            return Ok(key);
        }
        eprintln!("attempt {attempt}: no change event for {key} yet; renaming the record");
    }
    bail!("no keyed change event reached the journal after {KEYED_ATTEMPTS} attempts")
}

async fn dead_letters_for(pool: &deadpool_postgres::Pool, record_id: &str) -> Result<Vec<String>> {
    Ok(pool
        .get()
        .await?
        .query(
            "select error_message from sync_dead_letter where payload::text like $1",
            &[&format!("%{record_id}%")],
        )
        .await?
        .iter()
        .map(|row| row.get(0))
        .collect())
}

async fn fresh_database() -> Result<deadpool_postgres::Pool> {
    let pool = support::postgres::test_pool();
    support::postgres::reset_schema(&pool).await?;
    force_sync::migrate(&pool).await?;
    Ok(pool)
}

// ─── Capture windows ───────────────────────────────────────────────────────

/// The create, sparse update and delete events of one record all reach the
/// journal under one key, and applying them links the record.
async fn capture_window_contract(
    tier: &CdcTier,
    pool: &deadpool_postgres::Pool,
    record: &mut LiveRecord<'_>,
    source: Arc<OpenedSource>,
    engine: SyncEngine<LiveAuth>,
) -> Result<()> {
    let stream = tier.stream();
    let capture = {
        let engine = engine.clone();
        let source = Arc::clone(&source);
        tokio::spawn(async move {
            loop {
                engine
                    .run_cdc_window(
                        source.as_ref(),
                        &stream,
                        CdcWindow {
                            max_duration: Duration::from_secs(15),
                            max_events: 200,
                        },
                    )
                    .await?;
            }
            #[allow(unreachable_code)]
            Ok::<(), ForceSyncError>(())
        })
    };

    let result = async {
        source.wait_until_open().await?;
        let key = write_until_captured(pool, tier, record).await?;
        let record_id = record.id()?.as_str().to_owned();

        let value = record.sparse_update().await?;
        let rows = wait_for_rows(pool, tier, &key, 2)
            .await?
            .context("the sparse update never reached the journal")?;
        let (operation, payload) = &rows[1];
        ensure!(operation == "upsert", "update row: {rows:?}");
        ensure!(payload[&tier.update_field] == json!(value), "{payload}");
        ensure!(payload["Id"] == json!(record_id), "{payload}");
        ensure!(
            payload.get(&tier.external_id_field).is_none(),
            "a sparse update carries no unchanged key, and capture drops its null: {payload}"
        );

        record.delete().await?;
        let rows = wait_for_rows(pool, tier, &key, 3)
            .await?
            .context("the delete never reached the journal")?;
        ensure!(rows[2].0 == "delete", "delete row: {rows:?}");

        let dead = dead_letters_for(pool, &record_id).await?;
        ensure!(
            dead.is_empty(),
            "events of the test record were quarantined: {dead:?}"
        );

        while engine.run_apply_once().await? > 0 {}
        let link = PgStore::new(pool.clone())
            .get_link(TENANT, &tier.object_name, &key)
            .await?
            .context("applying the captured changes must link the record")?;
        ensure!(
            link.salesforce_id.as_deref() == Some(record_id.as_str()),
            "{link:?}"
        );
        ensure!(
            link.tombstone,
            "the delete must tombstone the link: {link:?}"
        );
        Ok(())
    }
    .await;

    capture.abort();
    if let Ok(Ok(Err(error))) = tokio::time::timeout(Duration::from_secs(1), capture).await {
        eprintln!("capture window loop failed: {error}");
    }
    result
}

#[tokio::test]
#[ignore = "requires a live Salesforce org with CDC, SF_SYNC_CDC=1 and FORCE_SYNC_TEST_DATABASE_URL"]
async fn live_cdc_window_captures_a_create_a_sparse_update_and_a_delete() -> Result<()> {
    let Some(tier) =
        load_cdc_tier("live_cdc_window_captures_a_create_a_sparse_update_and_a_delete")?
    else {
        return Ok(());
    };
    let pool = fresh_database().await?;
    let client = tier.client().await?;
    let source = tier.source(&client).await?;
    let engine = SyncEngine::builder(client.clone())
        .postgres(PgStore::new(pool.clone()))
        .object(tier.object())
        .build()?;

    let mut record = LiveRecord::new(&client, &tier);
    let result = capture_window_contract(&tier, &pool, &mut record, source, engine).await;
    record.cleanup().await;
    result
}

// ─── Durable runtime ───────────────────────────────────────────────────────

#[cfg(feature = "durable")]
mod durable {
    use super::*;

    use force_sync::durable::{DurableSync, DurableSyncConfig, harvest_pool, migrate_harvest};

    fn database_url() -> Result<String> {
        env_string("FORCE_SYNC_TEST_DATABASE_URL").context("FORCE_SYNC_TEST_DATABASE_URL")
    }

    /// Waits until the link of `key` matches `done`.
    async fn wait_for_link(
        pool: &deadpool_postgres::Pool,
        tier: &CdcTier,
        key: &str,
        done: impl Fn(&force_sync::SyncLink) -> bool,
    ) -> Result<force_sync::SyncLink> {
        let store = PgStore::new(pool.clone());
        let deadline = Instant::now() + CHANGE_TIMEOUT;
        loop {
            let link = store.get_link(TENANT, &tier.object_name, key).await?;
            if let Some(link) = link.filter(|link| done(link)) {
                return Ok(link);
            }
            ensure!(
                Instant::now() < deadline,
                "the link of {key} did not settle"
            );
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }

    /// The durable CDC stream workflow captures and applies real events.
    async fn durable_contract(
        tier: &CdcTier,
        pool: &deadpool_postgres::Pool,
        record: &mut LiveRecord<'_>,
        source: &OpenedSource,
    ) -> Result<()> {
        source.wait_until_open().await?;
        let key = write_until_captured(pool, tier, record).await?;
        let record_id = record.id()?.as_str().to_owned();
        let link = wait_for_link(pool, tier, &key, |link| link.salesforce_id.is_some()).await?;
        ensure!(
            link.salesforce_id.as_deref() == Some(record_id.as_str()),
            "{link:?}"
        );

        record.sparse_update().await?;
        wait_for_rows(pool, tier, &key, 2)
            .await?
            .context("the sparse update never reached the journal")?;

        record.delete().await?;
        wait_for_link(pool, tier, &key, |link| link.tombstone).await?;
        let dead = dead_letters_for(pool, &record_id).await?;
        ensure!(
            dead.is_empty(),
            "events of the test record were quarantined: {dead:?}"
        );
        Ok(())
    }

    #[tokio::test]
    #[ignore = "requires a live Salesforce org with CDC, SF_SYNC_CDC=1 and FORCE_SYNC_TEST_DATABASE_URL"]
    async fn live_durable_cdc_stream_captures_and_applies_real_events() -> Result<()> {
        let Some(tier) = load_cdc_tier("live_durable_cdc_stream_captures_and_applies_real_events")?
        else {
            return Ok(());
        };
        let pool = fresh_database().await?;
        let url = database_url()?;
        migrate_harvest(&url).await?;
        let client = tier.client().await?;
        let source = tier.source(&client).await?;
        let engine = SyncEngine::builder(client.clone())
            .postgres(PgStore::new(pool.clone()))
            .object(tier.object())
            .build()?;
        let runtime = DurableSync::new(engine, harvest_pool(&url, 8)?)
            .with_config(DurableSyncConfig {
                pump_interval: Duration::from_secs(1),
                reconcile_interval: None,
                cdc_window: CdcWindow {
                    max_duration: Duration::from_secs(15),
                    max_events: 200,
                },
                cdc_supervise_interval: Duration::from_secs(2),
                cdc_error_backoff: Duration::from_secs(2),
                ..DurableSyncConfig::default()
            })?
            .with_cdc(Arc::clone(&source) as Arc<dyn CdcSource>, [tier.stream()])?
            .start()?;

        let mut record = LiveRecord::new(&client, &tier);
        let result = durable_contract(&tier, &pool, &mut record, &source).await;
        record.cleanup().await;
        runtime.stop().await?;
        result
    }
}
