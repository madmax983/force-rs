//! Error types for force-sync.

/// Convenience Result alias for force-sync operations.
pub type Result<T> = std::result::Result<T, ForceSyncError>;

/// Database and migration errors.
#[derive(Debug, thiserror::Error)]
pub enum ForceSyncError {
    /// Postgres pool acquisition failure.
    #[error("database pool error: {0}")]
    Pool(#[from] deadpool_postgres::PoolError),

    /// Postgres client or query failure.
    #[error("database error: {0}")]
    Database(#[from] tokio_postgres::Error),

    /// A replay cursor is required to append journal entries.
    #[error("sync journal entries require a source cursor")]
    MissingSourceCursor,

    /// The transaction callback failed and the rollback also failed.
    #[error(
        "transaction callback failed and rollback also failed: callback={callback}; rollback={rollback}"
    )]
    TransactionRollback {
        /// Error returned by the transaction callback.
        callback: Box<Self>,
        /// Error returned while attempting to roll back the transaction.
        rollback: tokio_postgres::Error,
    },

    /// A required sync key part was empty.
    #[error("sync key {part} cannot be empty")]
    EmptySyncKeyPart {
        /// The offending sync key part.
        part: &'static str,
    },

    /// A requested sync record was not found.
    #[error("missing {entity}")]
    NotFound {
        /// The missing entity name.
        entity: &'static str,
    },

    /// A required engine configuration field was not provided.
    #[error("missing required configuration: {field}")]
    MissingConfiguration {
        /// The missing configuration field.
        field: &'static str,
    },

    /// A lease duration could not be represented as a Postgres interval timestamp.
    #[error("lease duration is out of range")]
    InvalidLeaseDuration,

    /// A JSON payload could not be decoded.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    /// A Salesforce Pub/Sub operation failed.
    #[error("pubsub error: {0}")]
    PubSub(Box<force_pubsub::PubSubError>),

    /// An outbox row carried an unexpected operation value.
    #[error("invalid outbox operation: {op}")]
    InvalidOutboxOperation {
        /// The invalid operation value.
        op: String,
    },

    /// An outbox row carried an already-encoded or otherwise invalid cursor.
    #[error("invalid outbox cursor: {cursor}")]
    InvalidOutboxCursor {
        /// The invalid cursor value.
        cursor: String,
    },

    /// A stored database value could not be decoded into a domain type.
    #[error("invalid stored value for {field}: {value}")]
    InvalidStoredValue {
        /// The field or column name.
        field: &'static str,
        /// The invalid stored value.
        value: String,
    },

    /// Placeholder variant while the crate surface is being implemented.
    #[error("not implemented")]
    NotImplemented,
}

impl From<force_pubsub::PubSubError> for ForceSyncError {
    fn from(error: force_pubsub::PubSubError) -> Self {
        Self::PubSub(Box::new(error))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pubsub_error_conversion() {
        let pubsub_err = force_pubsub::PubSubError::Config("test".to_string());
        let sync_err: ForceSyncError = pubsub_err.into();
        assert!(matches!(sync_err, ForceSyncError::PubSub(_)));
    }

    #[test]
    fn test_pool_error_conversion() {
        let pool_err = deadpool_postgres::PoolError::Closed;
        let sync_err: ForceSyncError = pool_err.into();
        assert!(matches!(sync_err, ForceSyncError::Pool(_)));
    }

    #[test]
    fn test_missing_config_display() {
        let err = ForceSyncError::MissingConfiguration { field: "tenant_id" };
        assert_eq!(err.to_string(), "missing required configuration: tenant_id");
    }

    #[test]
    fn test_missing_source_cursor_display() {
        let err = ForceSyncError::MissingSourceCursor;
        assert_eq!(
            err.to_string(),
            "sync journal entries require a source cursor"
        );
    }

    #[test]
    fn test_missing_not_found_display() {
        let err = ForceSyncError::NotFound { entity: "Account" };
        assert_eq!(err.to_string(), "missing Account");
    }

    #[test]
    fn test_invalid_lease_duration_display() {
        let err = ForceSyncError::InvalidLeaseDuration;
        assert_eq!(err.to_string(), "lease duration is out of range");
    }

    #[test]
    fn test_not_implemented_display() {
        let err = ForceSyncError::NotImplemented;
        assert_eq!(err.to_string(), "not implemented");
    }
}
