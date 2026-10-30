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
use autumn_harvest::worker::{DbPool, Worker, WorkerRuntimeConfig};
use chrono::Utc;
use force::auth::Authenticator;

use crate::{
    SyncEngine,
    apply::steps::{ApplyPlan, ApplyResolution, RemoteCall, RemoteOutcome},
    error::ForceSyncError,
};

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
            .lease_ready_tasks(lease_owner, limit, lease_for)
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

/// The shared state that durable sync activities read.
#[derive(Clone)]
pub(crate) struct DurableSyncState {
    pub(crate) ops: Arc<dyn SyncOps>,
    pub(crate) harvest_pool: DbPool,
    pub(crate) config: DurableSyncConfig,
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
    /// Wraps `engine` with the default [`DurableSyncConfig`].
    ///
    /// `harvest_pool` is the pool of the harvest database. It can be the
    /// force-sync database or a separate one. Apply the harvest migrations
    /// to it first, for example with [`migrate_harvest`].
    #[must_use]
    pub fn new<A: Authenticator + 'static>(engine: SyncEngine<A>, harvest_pool: DbPool) -> Self {
        Self {
            state: DurableSyncState {
                ops: Arc::new(engine),
                harvest_pool,
                config: DurableSyncConfig::default(),
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

    /// The active config.
    #[must_use]
    pub const fn config(&self) -> &DurableSyncConfig {
        &self.state.config
    }

    /// The pump and reconcile schedules, on [`FORCE_SYNC_QUEUE`].
    #[must_use]
    pub fn schedules(&self) -> Vec<WorkflowSchedule> {
        Self::schedules_for(&self.state.config)
    }

    fn schedules_for(config: &DurableSyncConfig) -> Vec<WorkflowSchedule> {
        let pump = Some((PUMP_WORKFLOW, config.pump_interval));
        let reconcile = config
            .reconcile_interval
            .map(|interval| (RECONCILE_WORKFLOW, interval));
        [pump, reconcile]
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
        let worker = Worker::new(
            WorkerRuntimeConfig::from(worker_config),
            Arc::clone(&registry),
        )
        .map_err(|error| ForceSyncError::Durable(format!("harvest worker: {error}")))?;
        let worker = Arc::new(worker);

        let pool = self.state.harvest_pool.clone();
        let running = Arc::clone(&worker);
        let worker_task = tokio::spawn(async move { running.run(&pool).await });
        let scheduler = SchedulerRuntime::spawn(
            self.state.harvest_pool,
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
