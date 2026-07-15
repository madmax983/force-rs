//! Error types for `force-lake`.
//!
//! [`LakeError`] is the crate's unified error type. It wraps the upstream
//! Salesforce client error ([`force::error::ForceError`]) alongside the Iceberg,
//! Arrow, Parquet, and serialization errors that arise while building and
//! committing analytics snapshots.

use thiserror::Error;

/// Convenient alias for results returned throughout `force-lake`.
pub type Result<T> = std::result::Result<T, LakeError>;

/// Unified error type for the analytics snapshot sink.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum LakeError {
    /// An error originating from the underlying Salesforce client.
    #[error("salesforce client error: {0}")]
    Force(#[from] force::error::ForceError),

    /// An error originating from the Apache Iceberg library.
    #[error("iceberg error: {0}")]
    Iceberg(#[from] iceberg::Error),

    /// An error originating from Arrow schema or array construction.
    #[error("arrow error: {0}")]
    Arrow(#[from] arrow_schema::ArrowError),

    /// An error originating from the Parquet reader or writer.
    #[error("parquet error: {0}")]
    Parquet(#[from] parquet::errors::ParquetError),

    /// A JSON (de)serialization error.
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    /// Invalid or incomplete sink configuration.
    #[error("configuration error: {0}")]
    Config(String),

    /// A Salesforce record could not be mapped into an Arrow column.
    #[error("record mapping error for column `{column}`: {message}")]
    RecordMapping {
        /// The Arrow column the mapping failed for.
        column: String,
        /// Human-readable detail of what went wrong.
        message: String,
    },
}

impl LakeError {
    /// Constructs a [`LakeError::Config`] from any displayable value.
    pub fn config(message: impl Into<String>) -> Self {
        Self::Config(message.into())
    }

    /// Constructs a [`LakeError::RecordMapping`] for a column.
    pub fn record_mapping(column: impl Into<String>, message: impl Into<String>) -> Self {
        Self::RecordMapping {
            column: column.into(),
            message: message.into(),
        }
    }
}
