//! Durable sync on the autumn-harvest workflow engine (feature: `durable`).
//!
//! The explicit runtime ([`SyncEngine::run_capture_postgres_once`] and its
//! siblings) leaves scheduling, retries and crash recovery to the caller.
//! This module runs the same pipeline as harvest workflows instead:
//!
//! - A **pump** schedule captures outbox rows into the journal, then
//!   dispatches each ready `sync_task` row as its own apply workflow.
//! - An **apply** workflow runs prepare, the Salesforce call, and finalize as
//!   three activities. Harvest records each result in its event log. A crash
//!   after the Salesforce call therefore resumes at finalize and does not
//!   repeat the call.
//! - A **reconcile** schedule runs drift detection and enqueues repairs.
//! - With [`DurableSync::with_cdc`](crate::durable::DurableSync::with_cdc), one long-running **CDC stream** workflow
//!   per Salesforce Change Data Capture stream follows that stream from its
//!   checkpoint. A **CDC supervisor** schedule keeps each one running.
//!
//! The force-sync tables stay the source of truth for sync state. Harvest
//! owns only scheduling, retries, and execution history. See ADR-036.
//!
//! ```rust,no_run
//! # use force_sync::{ObjectSync, PgStore, SyncEngine};
//! # use force_sync::durable::{DurableSync, harvest_pool, migrate_harvest};
//! # async fn run<A: force::auth::Authenticator + 'static>(
//! #     client: force::client::ForceClient<A>,
//! #     store: PgStore,
//! # ) -> Result<(), force_sync::ForceSyncError> {
//! let url = "postgres://localhost/harvest";
//! migrate_harvest(url).await?;
//!
//! let engine = SyncEngine::builder(client)
//!     .postgres(store)
//!     .object(ObjectSync::new("Account").external_id("ExternalId__c"))
//!     .build()?;
//!
//! let runtime = DurableSync::new(engine, harvest_pool(url, 16)?).start()?;
//! // ... run until shutdown ...
//! runtime.stop().await?;
//! # Ok(())
//! # }
//! ```

mod workflows;

use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use autumn_harvest::builder::{HarvestBuilder, WorkerConfig};
use autumn_harvest::policy::{RetryPolicy, Schedule, WorkflowSchedule};
use autumn_harvest::scheduler::{SchedulerRuntime, compile_dag_catalog};
use autumn_harvest::types::ExecutionId;
use autumn_harvest::worker::{DbPool, Worker, WorkerRuntimeConfig};
use chrono::Utc;
use force::auth::Authenticator;

use crate::{
    SyncEngine,
    apply::steps::{ApplyPlan, ApplyResolution, RemoteCall, RemoteOutcome},
    capture::salesforce::{CdcSource, CdcStream, CdcWindow, CdcWindowReport},
    error::ForceSyncError,
};

pub use autumn_harvest::shard::{ShardRouter, ShardedDbPool};
pub use autumn_harvest::types::ShardId;

/// The harvest task queue that every durable sync workflow and activity uses.
///
/// A harvest worker must poll this queue. [`DurableSync::start`] configures
/// its own worker to do so. With [`DurableSync::register`], add it to the
/// worker config of your app.
pub const FORCE_SYNC_QUEUE: &str = "force-sync";

/// The workflow that applies one `sync_task` row.
pub const APPLY_WORKFLOW: &str = workflows::APPLY_WORKFLOW;

/// The scheduled workflow that captures outbox rows and dispatches tasks.
pub const PUMP_WORKFLOW: &str = "force_sync_pump";

/// The scheduled workflow that runs reconciliation.
pub const RECONCILE_WORKFLOW: &str = "force_sync_reconcile";

/// The long-running workflow that follows one CDC stream.
pub const CDC_STREAM_WORKFLOW: &str = workflows::CDC_STREAM_WORKFLOW;

/// The scheduled workflow that keeps each CDC stream workflow running.
pub const CDC_SUPERVISOR_WORKFLOW: &str = "force_sync_cdc_supervisor";

/// The longest CDC window. The window activity has a 600 second timeout.
pub const MAX_CDC_WINDOW: Duration = Duration::from_secs(540);

/// The default lease owner of tasks that durable sync dispatches.
pub const DEFAULT_LEASE_OWNER: &str = "force-sync-durable";

/// Tuning for durable sync.
#[derive(Debug, Clone)]
pub struct DurableSyncConfig {
    /// Time between the end of one pump run and the start of the next.
    ///
    /// Must be a whole number of seconds, at least one. Default: 5 seconds.
    pub pump_interval: Duration,
    /// Time between reconciliation runs. `None` registers no reconcile
    /// schedule. Must be a whole number of seconds. Default: 5 minutes.
    pub reconcile_interval: Option<Duration>,
    /// The largest number of outbox batches one pump run captures.
    /// Default: 10.
    pub capture_batches_per_pump: u32,
    /// The largest number of tasks one pump run dispatches. Default: 100.
    pub dispatch_batch_size: i64,
    /// How long a dispatched task stays leased.
    ///
    /// A task whose apply workflow is still running when the lease expires is
    /// dispatched again, and attaches to the same workflow. A task whose
    /// workflow failed is dispatched again with a fresh run. Default: 1 hour.
    pub dispatch_lease: Duration,
    /// The retry policy of the Salesforce call. Only transient errors retry.
    /// Default: 8 attempts, exponential from 2 seconds.
    pub remote_retry: RetryPolicy,
    /// The lease owner of dispatched tasks. Default: [`DEFAULT_LEASE_OWNER`].
    ///
    /// Do not run the explicit runtime on the same store with this owner.
    pub lease_owner: String,
    /// The bounds of one CDC capture window. Default: 50 seconds or 1000
    /// events. `max_duration` can be at most [`MAX_CDC_WINDOW`].
    pub cdc_window: CdcWindow,
    /// Time between CDC supervisor runs. Must be a whole number of seconds.
    /// Default: 30 seconds.
    pub cdc_supervise_interval: Duration,
    /// Windows that one CDC stream run captures before it continues as new.
    /// It bounds the run history. Default: 60.
    pub cdc_windows_per_run: u32,
    /// Wait after a failed CDC window before the next one. Must be a whole
    /// number of seconds. Default: 10 seconds.
    pub cdc_error_backoff: Duration,
}

impl Default for DurableSyncConfig {
    fn default() -> Self {
        Self {
            pump_interval: Duration::from_secs(5),
            reconcile_interval: Some(Duration::from_secs(300)),
            capture_batches_per_pump: 10,
            dispatch_batch_size: 100,
            dispatch_lease: Duration::from_secs(3600),
            remote_retry: RetryPolicy::exponential(8, Duration::from_secs(2)),
            lease_owner: DEFAULT_LEASE_OWNER.to_owned(),
            cdc_window: CdcWindow::default(),
            cdc_supervise_interval: Duration::from_secs(30),
            cdc_windows_per_run: 60,
            cdc_error_backoff: Duration::from_secs(10),
        }
    }
}

impl DurableSyncConfig {
    /// Checks the config against the limits that harvest and the store apply.
    ///
    /// # Errors
    ///
    /// Returns [`ForceSyncError::Durable`] that names the first invalid field.
    pub fn validate(&self) -> Result<(), ForceSyncError> {
        let invalid = |message: &str| Err(ForceSyncError::Durable(message.to_owned()));
        if !is_whole_seconds(self.pump_interval) {
            return invalid("pump_interval must be a whole number of seconds, at least one");
        }
        if self
            .reconcile_interval
            .is_some_and(|interval| !is_whole_seconds(interval))
        {
            return invalid("reconcile_interval must be a whole number of seconds, at least one");
        }
        if self.capture_batches_per_pump == 0 {
            return invalid("capture_batches_per_pump must be at least one");
        }
        if self.dispatch_batch_size <= 0 {
            return invalid("dispatch_batch_size must be at least one");
        }
        if self.dispatch_lease.is_zero() {
            return invalid("dispatch_lease must be greater than zero");
        }
        if self.remote_retry.max_attempts == 0 {
            return invalid("remote_retry.max_attempts must be at least one");
        }
        if self.lease_owner.is_empty() {
            return invalid("lease_owner cannot be empty");
        }
        if self.cdc_window.max_duration.is_zero() || self.cdc_window.max_duration > MAX_CDC_WINDOW {
            return invalid("cdc_window.max_duration must be between zero and MAX_CDC_WINDOW");
        }
        if self.cdc_window.max_events == 0 {
            return invalid("cdc_window.max_events must be at least one");
        }
        if !is_whole_seconds(self.cdc_supervise_interval) {
            return invalid(
                "cdc_supervise_interval must be a whole number of seconds, at least one",
            );
        }
        if self.cdc_windows_per_run == 0 {
            return invalid("cdc_windows_per_run must be at least one");
        }
        if !is_whole_seconds(self.cdc_error_backoff) {
            return invalid("cdc_error_backoff must be a whole number of seconds, at least one");
        }
        Ok(())
    }
}

const fn is_whole_seconds(interval: Duration) -> bool {
    interval.subsec_nanos() == 0 && interval.as_secs() > 0
}

/// The engine operations that the activities call.
///
/// It erases the authenticator type of [`SyncEngine`], because harvest
/// activities are free functions that read their dependencies from shared
/// state.
#[async_trait]
pub(crate) trait SyncOps: Send + Sync {
    fn capture_batch_size(&self) -> i64;
    fn has_object(&self, object_name: &str) -> bool;
    async fn cdc_window(
        &self,
        source: &dyn CdcSource,
        stream: &CdcStream,
        window: CdcWindow,
    ) -> Result<CdcWindowReport, ForceSyncError>;
    async fn capture_postgres_once(&self) -> Result<usize, ForceSyncError>;
    async fn reconcile_once(&self) -> Result<usize, ForceSyncError>;
    async fn lease_apply_tasks(
        &self,
        lease_owner: &str,
        limit: i64,
        lease_for: Duration,
    ) -> Result<Vec<(i64, Option<String>)>, ForceSyncError>;
    async fn release_tasks(
        &self,
        lease_owner: &str,
        task_ids: &[i64],
        reason: &str,
    ) -> Result<(), ForceSyncError>;
    async fn fail_task(
        &self,
        lease_owner: &str,
        task_id: i64,
        reason: &str,
    ) -> Result<(), ForceSyncError>;
    async fn prepare_apply(
        &self,
        task_id: i64,
        batch_size: usize,
    ) -> Result<ApplyPlan, ForceSyncError>;
    async fn execute_remote(&self, call: &RemoteCall) -> RemoteOutcome;
    async fn finalize_apply(
        &self,
        lease_owner: &str,
        task_id: i64,
        resolution: &ApplyResolution,
    ) -> Result<bool, ForceSyncError>;
}

#[async_trait]
impl<A: Authenticator + 'static> SyncOps for SyncEngine<A> {
    fn capture_batch_size(&self) -> i64 {
        self.capture_batch_size_value()
    }

    fn has_object(&self, object_name: &str) -> bool {
        self.object(object_name).is_some()
    }

    async fn cdc_window(
        &self,
        source: &dyn CdcSource,
        stream: &CdcStream,
        window: CdcWindow,
    ) -> Result<CdcWindowReport, ForceSyncError> {
        self.run_cdc_window(source, stream, window).await
    }

    async fn capture_postgres_once(&self) -> Result<usize, ForceSyncError> {
        self.run_capture_postgres_once().await
    }

    async fn reconcile_once(&self) -> Result<usize, ForceSyncError> {
        self.run_reconcile_once().await
    }

    async fn lease_apply_tasks(
        &self,
        lease_owner: &str,
        limit: i64,
        lease_for: Duration,
    ) -> Result<Vec<(i64, Option<String>)>, ForceSyncError> {
        let leased = self
            .store()
            .lease_ready_or_expired_tasks(lease_owner, limit, lease_for)
            .await?;
        if leased.is_empty() {
            return Ok(Vec::new());
        }
        let task_ids: Vec<i64> = leased.iter().map(|task| task.task_id).collect();
        Ok(self
            .store()
            .apply_task_keys(&task_ids)
            .await?
            .into_iter()
            .map(|(task_id, key)| (task_id, key.map(|key| concurrency_key(&key))))
            .collect())
    }

    async fn release_tasks(
        &self,
        lease_owner: &str,
        task_ids: &[i64],
        reason: &str,
    ) -> Result<(), ForceSyncError> {
        for task_id in task_ids {
            self.store()
                .retry_task_for_worker(lease_owner, *task_id, Utc::now(), reason)
                .await?;
        }
        Ok(())
    }

    async fn fail_task(
        &self,
        lease_owner: &str,
        task_id: i64,
        reason: &str,
    ) -> Result<(), ForceSyncError> {
        self.store()
            .fail_task_for_worker(lease_owner, task_id, reason)
            .await
            .map(|_| ())
    }

    async fn prepare_apply(
        &self,
        task_id: i64,
        batch_size: usize,
    ) -> Result<ApplyPlan, ForceSyncError> {
        Self::prepare_apply(self, task_id, batch_size).await
    }

    async fn execute_remote(&self, call: &RemoteCall) -> RemoteOutcome {
        Self::execute_remote(self, call).await
    }

    async fn finalize_apply(
        &self,
        lease_owner: &str,
        task_id: i64,
        resolution: &ApplyResolution,
    ) -> Result<bool, ForceSyncError> {
        Self::finalize_apply(self, lease_owner, task_id, resolution).await
    }
}

/// The harvest concurrency key of one sync record.
///
/// JSON keeps the three parts unambiguous when one of them holds a
/// separator character.
fn concurrency_key((tenant, object_name, external_id): &(String, String, String)) -> String {
    serde_json::json!([tenant, object_name, external_id]).to_string()
}

/// The CDC source and the streams that durable sync follows.
pub(crate) struct CdcSetup {
    pub(crate) source: Arc<dyn CdcSource>,
    pub(crate) streams: Vec<CdcStream>,
}

impl CdcSetup {
    /// The configured stream named `name`.
    pub(crate) fn stream(&self, name: &str) -> Option<&CdcStream> {
        self.streams.iter().find(|stream| stream.name() == name)
    }
}

/// The harvest databases and the router that places workflows on them.
#[derive(Clone)]
pub(crate) struct HarvestShards {
    pub(crate) pool: ShardedDbPool,
    pub(crate) router: ShardRouter,
}

impl HarvestShards {
    /// One harvest database: every workflow lands on shard 0.
    pub(crate) fn single(pool: DbPool) -> Self {
        Self {
            pool: ShardedDbPool::single(pool),
            router: ShardRouter::single(),
        }
    }

    /// Several harvest databases, placed by `router`.
    ///
    /// # Errors
    ///
    /// Returns [`ForceSyncError::Durable`] when the router can read a shard
    /// that `pool` has no pool for.
    pub(crate) fn new(pool: ShardedDbPool, router: ShardRouter) -> Result<Self, ForceSyncError> {
        if let Some(missing) = router
            .readable_shards()
            .iter()
            .find(|shard| pool.exact_pool_for(**shard).is_none())
        {
            return Err(ForceSyncError::Durable(format!(
                "the shard router names shard {missing}, but the sharded pool has no pool for it"
            )));
        }
        Ok(Self { pool, router })
    }

    /// The shard and a new execution ID for a fresh start of
    /// `(workflow_name, workflow_id)`.
    ///
    /// The router hashes the pair, so one workflow ID always lands on one
    /// shard. Harvest keeps workflow IDs unique per shard, so that is what
    /// keeps an idempotent start idempotent across shards.
    pub(crate) fn place(&self, workflow_name: &str, workflow_id: &str) -> (ShardId, ExecutionId) {
        let shard = self
            .router
            .pick_for_new_workflow(workflow_name, workflow_id);
        (shard, ExecutionId::new_for_shard(shard))
    }
}

/// The shared state that durable sync activities read.
#[derive(Clone)]
pub(crate) struct DurableSyncState {
    pub(crate) ops: Arc<dyn SyncOps>,
    pub(crate) harvest: HarvestShards,
    pub(crate) config: DurableSyncConfig,
    pub(crate) cdc: Option<Arc<CdcSetup>>,
}

/// Durable sync: a [`SyncEngine`] wired into autumn-harvest.
///
/// Use [`Self::start`] to run a standalone harvest worker and scheduler. Use
/// [`Self::register`] to add durable sync to a harvest app that already runs
/// a worker, such as one built with `HarvestPlugin`.
pub struct DurableSync {
    state: DurableSyncState,
}

impl std::fmt::Debug for DurableSync {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DurableSync")
            .field("config", &self.state.config)
            .finish_non_exhaustive()
    }
}

impl DurableSync {
    /// Wraps `engine` with the default [`DurableSyncConfig`], on one harvest
    /// database.
    ///
    /// `harvest_pool` is the pool of the harvest database. It can be the
    /// force-sync database or a separate one. Apply the harvest migrations
    /// to it first, for example with [`migrate_harvest`].
    #[must_use]
    pub fn new<A: Authenticator + 'static>(engine: SyncEngine<A>, harvest_pool: DbPool) -> Self {
        Self::with_shards(engine, HarvestShards::single(harvest_pool))
    }

    /// Wraps `engine` with the default [`DurableSyncConfig`], on several
    /// harvest databases (shards).
    ///
    /// `router` places each workflow on a shard by hashing its name and ID,
    /// so one apply task or one CDC stream always lands on one shard. The
    /// force-sync tables stay in the engine's own database. Apply the
    /// harvest migrations to every shard first, and build `pool` with
    /// [`harvest_sharded_pool`] or harvest's own `ShardedDbPool`.
    ///
    /// With [`Self::register`], pass the router and the pool that the app's
    /// harvest runtime uses, so durable sync starts its workflows on the
    /// shards that the app's workers poll.
    ///
    /// # Errors
    ///
    /// Returns [`ForceSyncError::Durable`] when `router` can read a shard
    /// that `pool` has no pool for.
    pub fn new_sharded<A: Authenticator + 'static>(
        engine: SyncEngine<A>,
        pool: ShardedDbPool,
        router: ShardRouter,
    ) -> Result<Self, ForceSyncError> {
        Ok(Self::with_shards(engine, HarvestShards::new(pool, router)?))
    }

    fn with_shards<A: Authenticator + 'static>(
        engine: SyncEngine<A>,
        harvest: HarvestShards,
    ) -> Self {
        Self {
            state: DurableSyncState {
                ops: Arc::new(engine),
                harvest,
                config: DurableSyncConfig::default(),
                cdc: None,
            },
        }
    }

    /// Replaces the config.
    ///
    /// # Errors
    ///
    /// Returns [`ForceSyncError::Durable`] when [`DurableSyncConfig::validate`]
    /// rejects `config`.
    pub fn with_config(mut self, config: DurableSyncConfig) -> Result<Self, ForceSyncError> {
        config.validate()?;
        self.state.config = config;
        Ok(self)
    }

    /// Follows Salesforce Change Data Capture `streams` from `source`.
    ///
    /// Each stream gets one long-running workflow, with ID
    /// `force-sync-cdc-{stream name}`, so one process in the fleet reads it at
    /// a time. A supervisor schedule restarts a stream workflow that failed.
    /// Each event commits its journal row, apply task and checkpoint in one
    /// transaction, so a restart resumes from the checkpoint.
    ///
    /// # Errors
    ///
    /// Returns [`ForceSyncError::Durable`] when `streams` is empty or two
    /// streams share a name, and [`ForceSyncError::MissingConfiguration`]
    /// when a stream names an object that the engine does not sync.
    pub fn with_cdc(
        mut self,
        source: Arc<dyn CdcSource>,
        streams: impl IntoIterator<Item = CdcStream>,
    ) -> Result<Self, ForceSyncError> {
        let streams: Vec<CdcStream> = streams.into_iter().collect();
        if streams.is_empty() {
            return Err(ForceSyncError::Durable(
                "with_cdc needs at least one stream".to_owned(),
            ));
        }
        let mut names = std::collections::BTreeSet::new();
        for stream in &streams {
            if !names.insert(stream.name()) {
                return Err(ForceSyncError::Durable(format!(
                    "two CDC streams share the name {}",
                    stream.name()
                )));
            }
            if !self.state.ops.has_object(stream.object_name()) {
                return Err(ForceSyncError::MissingConfiguration {
                    field: "object sync",
                });
            }
        }
        self.state.cdc = Some(Arc::new(CdcSetup { source, streams }));
        Ok(self)
    }

    /// The active config.
    #[must_use]
    pub const fn config(&self) -> &DurableSyncConfig {
        &self.state.config
    }

    /// The pump, reconcile and CDC supervisor schedules, on
    /// [`FORCE_SYNC_QUEUE`].
    #[must_use]
    pub fn schedules(&self) -> Vec<WorkflowSchedule> {
        Self::schedules_for(&self.state.config, self.state.cdc.is_some())
    }

    fn schedules_for(config: &DurableSyncConfig, cdc: bool) -> Vec<WorkflowSchedule> {
        let pump = Some((PUMP_WORKFLOW, config.pump_interval));
        let reconcile = config
            .reconcile_interval
            .map(|interval| (RECONCILE_WORKFLOW, interval));
        let supervisor = cdc.then_some((CDC_SUPERVISOR_WORKFLOW, config.cdc_supervise_interval));
        [pump, reconcile, supervisor]
            .into_iter()
            .flatten()
            .map(|(workflow, interval)| WorkflowSchedule {
                queue_name: FORCE_SYNC_QUEUE.to_owned(),
                ..WorkflowSchedule::new(workflow, Schedule::Interval(interval))
            })
            .collect()
    }

    /// Adds the durable sync workflows, activities, schedules and state to
    /// `builder`.
    ///
    /// The worker that runs them must poll [`FORCE_SYNC_QUEUE`].
    #[must_use]
    pub fn register(&self, builder: HarvestBuilder) -> HarvestBuilder {
        self.schedules().into_iter().fold(
            builder
                .workflows(workflows::all_workflows())
                .activities(workflows::all_activities())
                .state(self.state.clone()),
            HarvestBuilder::workflow_schedule,
        )
    }

    /// Starts a harvest worker on [`FORCE_SYNC_QUEUE`] and the harvest
    /// scheduler, in the current Tokio runtime.
    ///
    /// # Errors
    ///
    /// Returns [`ForceSyncError::Durable`] when the harvest registration does
    /// not validate or the worker cannot be built.
    pub fn start(self) -> Result<DurableSyncRuntime, ForceSyncError> {
        let builder = self.register(
            HarvestBuilder::default()
                .worker(WorkerConfig::default().with_queues([FORCE_SYNC_QUEUE])),
        );
        let built = builder
            .try_build()
            .map_err(|error| ForceSyncError::Durable(format!("harvest build: {error}")))?;
        let (registry, dags, workflow_schedules, worker_config) = built.into_worker_parts();
        let registry = Arc::new(registry);
        let dag_catalog = compile_dag_catalog(dags)
            .map_err(|error| ForceSyncError::Durable(format!("harvest DAGs: {error}")))?;
        let HarvestShards { pool, router } = self.state.harvest;
        let mut runtime_config = WorkerRuntimeConfig::from(worker_config);
        // An empty assignment list makes the worker claim from every shard
        // of the pool.
        runtime_config.sharded_pool = Some(pool.clone());
        let worker = Worker::new(runtime_config, Arc::clone(&registry))
            .map_err(|error| ForceSyncError::Durable(format!("harvest worker: {error}")))?;
        let missing = worker.missing_assigned_shard_pools();
        if !missing.is_empty() {
            return Err(ForceSyncError::Durable(format!(
                "harvest worker: no pool for shards {missing:?}"
            )));
        }
        let worker = Arc::new(worker);

        // Harvest reads the process router and pool to route work between
        // shards, as the harvest plugin runtime sets them.
        autumn_harvest::shard::install_global_router(router.clone());
        autumn_harvest::shard::install_global_sharded_pool(pool.clone());

        let default_pool = pool.pool_for(router.parts().default_shard).clone();
        let running = Arc::clone(&worker);
        let worker_task = tokio::spawn(async move { running.run(&default_pool).await });
        let scheduler = SchedulerRuntime::spawn_sharded(
            pool,
            router,
            registry,
            Arc::new(dag_catalog),
            Arc::new(workflow_schedules),
        );

        Ok(DurableSyncRuntime {
            worker,
            worker_task,
            scheduler,
        })
    }
}

/// A running durable sync worker and scheduler.
///
/// Call [`Self::stop`] on shutdown. It drains the worker, so an in-flight
/// activity can finish and record its result.
pub struct DurableSyncRuntime {
    worker: Arc<Worker>,
    worker_task: tokio::task::JoinHandle<()>,
    scheduler: SchedulerRuntime,
}

impl std::fmt::Debug for DurableSyncRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DurableSyncRuntime")
            .field("worker_id", &self.worker.config.worker_id)
            .finish_non_exhaustive()
    }
}

impl DurableSyncRuntime {
    /// Stops the scheduler, drains the worker, and waits for both.
    ///
    /// # Errors
    ///
    /// Returns [`ForceSyncError::Durable`] when a background task panicked.
    pub async fn stop(self) -> Result<(), ForceSyncError> {
        self.scheduler.shutdown();
        self.worker.shutdown();
        let scheduler = self.scheduler.join().await;
        let worker = self.worker_task.await;
        scheduler
            .and(worker)
            .map_err(|error| ForceSyncError::Durable(format!("harvest task panicked: {error}")))
    }
}

/// Builds a harvest connection pool with the harvest engine timeouts.
///
/// # Errors
///
/// Returns [`ForceSyncError::Durable`] when the pool cannot be built.
pub fn harvest_pool(database_url: &str, max_size: usize) -> Result<DbPool, ForceSyncError> {
    autumn_harvest::pool::engine_pool(
        database_url,
        max_size,
        autumn_harvest::pool::DbRole::Hot,
        &autumn_harvest::pool::EngineDbTimeouts::default(),
    )
    .map_err(|error| ForceSyncError::Durable(format!("harvest pool: {error}")))
}

/// Builds a sharded harvest pool, one engine pool per shard DSN.
///
/// Each shard gets the harvest engine timeouts and at most `max_size`
/// connections. `default_shard` serves execution IDs that encode no shard.
///
/// # Errors
///
/// Returns [`ForceSyncError::Durable`] when `shards` is empty, does not
/// contain `default_shard`, or a pool cannot be built.
pub fn harvest_sharded_pool(
    shards: impl IntoIterator<Item = (ShardId, String)>,
    default_shard: ShardId,
    max_size: usize,
) -> Result<ShardedDbPool, ForceSyncError> {
    let mut pools = std::collections::BTreeMap::new();
    for (shard, url) in shards {
        pools.insert(shard, harvest_pool(&url, max_size)?);
    }
    if !pools.contains_key(&default_shard) {
        return Err(ForceSyncError::Durable(format!(
            "the default shard {default_shard} has no DSN"
        )));
    }
    Ok(ShardedDbPool::from_map(pools, default_shard))
}

/// Applies every pending harvest migration to `database_url`.
///
/// It is safe to run on every boot. Applied migrations are skipped.
///
/// # Errors
///
/// Returns [`ForceSyncError::Durable`] when the connection or a migration
/// fails.
pub async fn migrate_harvest(database_url: &str) -> Result<(), ForceSyncError> {
    let mut conn = autumn_harvest::pg_tls::connect(database_url)
        .await
        .map_err(|error| ForceSyncError::Durable(format!("harvest connect: {error}")))?;
    autumn_harvest::migrate::apply_to_connection(&mut conn, &autumn_harvest::migrate::embedded())
        .await
        .map(|_| ())
        .map_err(|error| ForceSyncError::Durable(format!("harvest migrate: {error}")))
}

#[cfg(test)]
mod tests;
