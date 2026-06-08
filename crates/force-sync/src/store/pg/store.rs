//! PostgreSQL-backed sync store helpers.

use std::future::Future;

use deadpool_postgres::{Client, Pool};
use futures::future::BoxFuture;

use crate::error::ForceSyncError;

/// PostgreSQL-backed store for sync engine work.
#[derive(Clone, Debug)]
pub struct PgStore {
    pool: Pool,
}

impl PgStore {
    /// Creates a new store wrapper around an existing connection pool.
    #[must_use]
    pub const fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// Returns the backing `PostgreSQL` pool.
    #[must_use]
    pub const fn pool(&self) -> &Pool {
        &self.pool
    }

    /// Runs an operation against a pooled `PostgreSQL` client.
    ///
    /// # Errors
    ///
    /// Returns a pool or query error if the client cannot be acquired or the
    /// callback fails.
    pub async fn with_client<T, F, Fut>(&self, f: F) -> crate::error::Result<T>
    where
        F: FnOnce(Client) -> Fut,
        Fut: Future<Output = Result<T, tokio_postgres::Error>>,
    {
        let client = self.pool.get().await?;
        f(client).await.map_err(Into::into)
    }

    /// Runs an operation inside a `PostgreSQL` transaction.
    ///
    /// # Errors
    ///
    /// Returns a pool or query error if the transaction cannot be opened or
    /// the callback fails.
    pub async fn with_transaction<T, F>(&self, f: F) -> crate::error::Result<T>
    where
        F: for<'a> FnOnce(
            &'a tokio_postgres::Transaction<'a>,
        ) -> BoxFuture<'a, Result<T, ForceSyncError>>,
    {
        let mut client = self.pool.get().await?;
        let transaction = client.transaction().await?;

        match f(&transaction).await {
            Ok(value) => {
                transaction.commit().await?;
                Ok(value)
            }
            Err(err) => match transaction.rollback().await {
                Ok(()) => Err(err),
                Err(rollback) => Err(ForceSyncError::TransactionRollback {
                    callback: Box::new(err),
                    rollback,
                }),
            },
        }
    }
}
