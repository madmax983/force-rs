//! Batch Query Processing.
//!
//! This module provides a utility to query records and perform batch operations on them.
//! It handles pagination and batch execution automatically.

use crate::api::rest_operation::RestOperation;
use crate::auth::Authenticator;
#[cfg(feature = "composite")]
use crate::client::ForceClient;
use crate::error::Result;
use serde::de::DeserializeOwned;
use serde_json::Value;

/// Operations supported by the batch processor.
#[derive(Debug, Clone)]
pub enum BatchOp {
    /// Update a record (SObject, Id, Fields).
    Update(String, String, Value),
    /// Delete a record (SObject, Id).
    Delete(String, String),
    /// Create a new record (SObject, Fields).
    Create(String, Value),
}

/// Statistics for the batch processing job.
#[derive(Debug, Default, Clone, Copy)]
pub struct BatchStats {
    /// Number of records processed from the query.
    pub records_processed: usize,
    /// Number of batch operations executed successfully.
    pub ops_succeeded: usize,
    /// Number of batch operations failed.
    pub ops_failed: usize,
}

/// A processor that queries records and executes batch operations.
#[cfg(feature = "composite")]
pub struct QueryBatch<'a, A: Authenticator> {
    client: &'a ForceClient<A>,
    query: String,
    halt_on_error: bool,
}

#[cfg(feature = "composite")]
impl<'a, A: Authenticator> QueryBatch<'a, A> {
    /// Creates a new query batch processor.
    ///
    /// # Arguments
    ///
    /// * `client` - The Force client.
    /// * `query` - The SOQL query to execute.
    pub fn new(client: &'a ForceClient<A>, query: impl Into<String>) -> Self {
        Self {
            client,
            query: query.into(),
            halt_on_error: false,
        }
    }

    /// Sets whether to stop processing if a batch operation fails.
    #[must_use]
    pub fn halt_on_error(mut self, halt: bool) -> Self {
        self.halt_on_error = halt;
        self
    }

    /// Executes the query and processes records.
    ///
    /// # Arguments
    ///
    /// * `transform` - A closure that takes a record and returns an optional operation.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails or if batch execution fails.
    pub async fn run<T, F>(self, transform: F) -> Result<BatchStats>
    where
        T: DeserializeOwned,
        F: Fn(T) -> Option<BatchOp>,
    {
        let mut stats = BatchStats::default();
        let mut buffer = Vec::with_capacity(25);

        let mut result = self.client.rest().query::<T>(&self.query).await?;

        loop {
            let records = std::mem::take(&mut result.records);
            for record in records {
                stats.records_processed += 1;
                if let Some(op) = transform(record) {
                    buffer.push(op);
                    if buffer.len() >= 25 {
                        self.flush_batch(buffer, &mut stats).await?;
                        buffer = Vec::with_capacity(25);
                    }
                }
            }

            if result.is_done() {
                break;
            }

            if let Some(next_url) = result.next_records_url {
                result = self.client.rest().query_more(&next_url).await?;
            } else {
                break;
            }
        }

        // Flush remaining
        if !buffer.is_empty() {
            self.flush_batch(buffer, &mut stats).await?;
        }

        Ok(stats)
    }

    async fn flush_batch(&self, ops: Vec<BatchOp>, stats: &mut BatchStats) -> Result<()> {
        if ops.is_empty() {
            return Ok(());
        }

        let mut batch = self
            .client
            .composite()
            .batch()
            .halt_on_error(self.halt_on_error);

        for op in ops {
            batch = match op {
                BatchOp::Update(sobject, id, fields) => batch.patch(&sobject, &id, fields)?,
                BatchOp::Delete(sobject, id) => batch.delete(&sobject, &id)?,
                BatchOp::Create(sobject, fields) => batch.post(&sobject, fields)?,
            };
        }

        let response = batch.execute().await?;

        for res in response.results {
            if (200..=299).contains(&res.status_code) {
                stats.ops_succeeded += 1;
            } else {
                stats.ops_failed += 1;
            }
        }

        Ok(())
    }
}
