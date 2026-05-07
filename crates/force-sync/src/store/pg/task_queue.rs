//! Task queue helpers for the `PostgreSQL` sync store.

use std::convert::TryFrom;
use std::time::Duration;

use chrono::{DateTime, Utc};
use tokio_postgres::GenericClient;

use crate::error::ForceSyncError;

use super::PgStore;

/// A leased task returned from the task queue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeasedTask {
    /// The leased task ID.
    pub task_id: i64,
    /// The worker that owns the lease.
    pub lease_owner: String,
    /// When the lease expires.
    pub lease_until: DateTime<Utc>,
}

struct LeaseDeadline {
    lease_until: DateTime<Utc>,
}

fn compute_lease_deadline(lease_for: Duration) -> Result<LeaseDeadline, ForceSyncError> {
    let secs =
        i64::try_from(lease_for.as_secs()).map_err(|_| ForceSyncError::InvalidLeaseDuration)?;
    let nanos = i64::from(lease_for.subsec_nanos());

    let duration_secs =
        chrono::Duration::try_seconds(secs).ok_or(ForceSyncError::InvalidLeaseDuration)?;
    let duration_nanos = chrono::Duration::nanoseconds(nanos);

    let duration = duration_secs
        .checked_add(&duration_nanos)
        .ok_or(ForceSyncError::InvalidLeaseDuration)?;

    let lease_until = Utc::now()
        .checked_add_signed(duration)
        .ok_or(ForceSyncError::InvalidLeaseDuration)?;

    Ok(LeaseDeadline { lease_until })
}

async fn enqueue_apply_task_query<C>(
    client: &C,
    journal_id: i64,
    priority: i32,
) -> Result<i64, ForceSyncError>
where
    C: GenericClient + Sync + ?Sized,
{
    let journal_key = journal_id.to_string();
    let row = client
        .query_one(
            "insert into sync_task (status, task_kind, target_key, priority, payload)
             values ('ready', 'apply', $1::text, $2, jsonb_build_object('journal_id', $1))
             returning task_id",
            &[&journal_key, &priority],
        )
        .await?;

    Ok(row.get(0))
}

async fn lease_ready_tasks_query<C>(
    client: &C,
    worker_id: &str,
    limit: i64,
    lease_until: &LeaseDeadline,
) -> Result<Vec<LeasedTask>, ForceSyncError>
where
    C: GenericClient + Sync + ?Sized,
{
    let rows = client
        .query(
            "with claimed as (
                select task_id
                from sync_task
                where status = 'ready'
                  and next_attempt_at <= now()
                  and (lease_until is null or lease_until <= now())
                order by priority desc, next_attempt_at asc, task_id asc
                for update skip locked
                limit $2
            )
            update sync_task
            set status = 'leased',
                lease_owner = $1,
                lease_until = $3::timestamptz,
                attempt_count = attempt_count + 1,
                updated_at = now()
            from claimed
            where sync_task.task_id = claimed.task_id
            returning sync_task.task_id",
            &[&worker_id, &limit, &lease_until.lease_until],
        )
        .await?;

    Ok(rows
        .into_iter()
        .map(|row| LeasedTask {
            task_id: row.get(0),
            lease_owner: worker_id.to_owned(),
            lease_until: lease_until.lease_until,
        })
        .collect())
}

async fn update_task_status_unguarded<C>(
    client: &C,
    task_id: i64,
    status: &str,
    last_error: Option<&str>,
    next_attempt_at: Option<DateTime<Utc>>,
) -> Result<u64, ForceSyncError>
where
    C: GenericClient + Sync + ?Sized,
{
    Ok(client
        .execute(
            "update sync_task
                 set status = $2,
                     last_error = $3,
                     next_attempt_at = coalesce($4::timestamptz, 'now'::timestamptz),
                     lease_owner = null,
                     lease_until = null,
                     updated_at = now()
                 where task_id = $1",
            &[&task_id, &status, &last_error, &next_attempt_at],
        )
        .await?)
}

async fn update_task_status_guarded<C>(
    client: &C,
    task_id: i64,
    status: &str,
    last_error: Option<&str>,
    next_attempt_at: Option<DateTime<Utc>>,
    worker_id: &str,
) -> Result<u64, ForceSyncError>
where
    C: GenericClient + Sync + ?Sized,
{
    Ok(client
        .execute(
            "update sync_task
                 set status = $2,
                     last_error = $3,
                     next_attempt_at = coalesce($4::timestamptz, 'now'::timestamptz),
                     lease_owner = null,
                     lease_until = null,
                     updated_at = now()
                 where task_id = $1
                   and status = 'leased'
                   and lease_owner = $5",
            &[&task_id, &status, &last_error, &next_attempt_at, &worker_id],
        )
        .await?)
}

async fn update_task_status<C>(
    client: &C,
    task_id: i64,
    status: &str,
    last_error: Option<&str>,
    next_attempt_at: Option<DateTime<Utc>>,
    expected_lease_owner: Option<&str>,
) -> Result<u64, ForceSyncError>
where
    C: GenericClient + Sync + ?Sized,
{
    match expected_lease_owner {
        Some(worker_id) => {
            update_task_status_guarded(
                client,
                task_id,
                status,
                last_error,
                next_attempt_at,
                worker_id,
            )
            .await
        }
        None => {
            update_task_status_unguarded(client, task_id, status, last_error, next_attempt_at).await
        }
    }
}

impl PgStore {
    /// Enqueues an apply task for a journal row.
    ///
    /// # Errors
    ///
    /// Returns an error if the database write fails.
    pub async fn enqueue_apply_task(
        &self,
        journal_id: i64,
        priority: i32,
    ) -> Result<i64, ForceSyncError> {
        let client = self.pool().get().await?;
        enqueue_apply_task_query(&**client, journal_id, priority).await
    }

    /// Leases ready tasks for a worker.
    ///
    /// # Errors
    ///
    /// Returns an error if the lease duration is invalid or the database write fails.
    pub async fn lease_ready_tasks(
        &self,
        worker_id: &str,
        limit: i64,
        lease_for: Duration,
    ) -> Result<Vec<LeasedTask>, ForceSyncError> {
        let lease_deadline = compute_lease_deadline(lease_for)?;
        let client = self.pool().get().await?;
        lease_ready_tasks_query(&**client, worker_id, limit, &lease_deadline).await
    }

    /// Acknowledges a completed task administratively, without lease-owner checks.
    ///
    /// # Errors
    ///
    /// Returns an error if the database write fails.
    pub async fn ack_task(&self, task_id: i64) -> Result<u64, ForceSyncError> {
        let client = self.pool().get().await?;
        update_task_status(&**client, task_id, "done", None, None, None).await
    }

    /// Acknowledges a completed task for the current lease owner.
    ///
    /// Returns `0` if the task is no longer leased by `worker_id`.
    ///
    /// # Errors
    ///
    /// Returns an error if the database write fails.
    pub async fn ack_task_for_worker(
        &self,
        worker_id: &str,
        task_id: i64,
    ) -> Result<u64, ForceSyncError> {
        let client = self.pool().get().await?;
        update_task_status(&**client, task_id, "done", None, None, Some(worker_id)).await
    }

    /// Marks a task for retry at a future time administratively, without lease-owner checks.
    ///
    /// # Errors
    ///
    /// Returns an error if the database write fails.
    pub async fn retry_task(
        &self,
        task_id: i64,
        next_attempt_at: DateTime<Utc>,
        error: impl AsRef<str>,
    ) -> Result<u64, ForceSyncError> {
        let error = error.as_ref().to_owned();
        let client = self.pool().get().await?;
        update_task_status(
            &**client,
            task_id,
            "ready",
            Some(&error),
            Some(next_attempt_at),
            None,
        )
        .await
    }

    /// Marks a task for retry for the current lease owner.
    ///
    /// Returns `0` if the task is no longer leased by `worker_id`.
    ///
    /// # Errors
    ///
    /// Returns an error if the database write fails.
    pub async fn retry_task_for_worker(
        &self,
        worker_id: &str,
        task_id: i64,
        next_attempt_at: DateTime<Utc>,
        error: impl AsRef<str>,
    ) -> Result<u64, ForceSyncError> {
        let error = error.as_ref().to_owned();
        let client = self.pool().get().await?;
        update_task_status(
            &**client,
            task_id,
            "ready",
            Some(&error),
            Some(next_attempt_at),
            Some(worker_id),
        )
        .await
    }

    /// Marks a task as failed administratively, without lease-owner checks.
    ///
    /// # Errors
    ///
    /// Returns an error if the database write fails.
    pub async fn fail_task(
        &self,
        task_id: i64,
        error: impl AsRef<str>,
    ) -> Result<u64, ForceSyncError> {
        let error = error.as_ref().to_owned();
        let client = self.pool().get().await?;
        update_task_status(&**client, task_id, "failed", Some(&error), None, None).await
    }

    /// Marks a task as failed for the current lease owner.
    ///
    /// Returns `0` if the task is no longer leased by `worker_id`.
    ///
    /// # Errors
    ///
    /// Returns an error if the database write fails.
    pub async fn fail_task_for_worker(
        &self,
        worker_id: &str,
        task_id: i64,
        error: impl AsRef<str>,
    ) -> Result<u64, ForceSyncError> {
        let error = error.as_ref().to_owned();
        let client = self.pool().get().await?;
        update_task_status(
            &**client,
            task_id,
            "failed",
            Some(&error),
            None,
            Some(worker_id),
        )
        .await
    }

    /// Enqueues an apply task inside an existing transaction.
    ///
    /// # Errors
    ///
    /// Returns an error if the database write fails.
    pub async fn enqueue_apply_task_in_tx<C>(
        client: &C,
        journal_id: i64,
        priority: i32,
    ) -> Result<i64, ForceSyncError>
    where
        C: GenericClient + Sync + ?Sized,
    {
        enqueue_apply_task_query(client, journal_id, priority).await
    }

    /// Leases ready tasks inside an existing transaction.
    ///
    /// # Errors
    ///
    /// Returns an error if the lease duration is invalid or the database write fails.
    pub async fn lease_ready_tasks_in_tx<C>(
        client: &C,
        worker_id: &str,
        limit: i64,
        lease_for: Duration,
    ) -> Result<Vec<LeasedTask>, ForceSyncError>
    where
        C: GenericClient + Sync + ?Sized,
    {
        let lease_deadline = compute_lease_deadline(lease_for)?;
        lease_ready_tasks_query(client, worker_id, limit, &lease_deadline).await
    }

    /// Acknowledges a completed task inside an existing transaction.
    ///
    /// # Errors
    ///
    /// Returns an error if the database write fails.
    pub async fn ack_task_in_tx<C>(client: &C, task_id: i64) -> Result<u64, ForceSyncError>
    where
        C: GenericClient + Sync + ?Sized,
    {
        update_task_status(client, task_id, "done", None, None, None).await
    }

    /// Marks a task for retry inside an existing transaction.
    ///
    /// # Errors
    ///
    /// Returns an error if the database write fails.
    pub async fn retry_task_in_tx<C>(
        client: &C,
        task_id: i64,
        next_attempt_at: DateTime<Utc>,
        error: impl AsRef<str>,
    ) -> Result<u64, ForceSyncError>
    where
        C: GenericClient + Sync + ?Sized,
    {
        let error = error.as_ref().to_owned();
        update_task_status(
            client,
            task_id,
            "ready",
            Some(&error),
            Some(next_attempt_at),
            None,
        )
        .await
    }

    /// Marks a task as failed inside an existing transaction.
    ///
    /// # Errors
    ///
    /// Returns an error if the database write fails.
    pub async fn fail_task_in_tx<C>(
        client: &C,
        task_id: i64,
        error: impl AsRef<str>,
    ) -> Result<u64, ForceSyncError>
    where
        C: GenericClient + Sync + ?Sized,
    {
        let error = error.as_ref().to_owned();
        update_task_status(client, task_id, "failed", Some(&error), None, None).await
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use chrono::Utc;

    use super::compute_lease_deadline;

    #[test]
    fn lease_deadline_keeps_datetime_type() {
        let before = Utc::now();
        let deadline = compute_lease_deadline(Duration::from_secs(5))
            .unwrap_or_else(|error| panic!("unexpected lease deadline error: {error}"));
        let after = Utc::now();

        let _: chrono::DateTime<chrono::Utc> = deadline.lease_until;
        assert!(deadline.lease_until >= before);
        assert!(deadline.lease_until <= after + chrono::Duration::seconds(5));
    }
}
