//! Error types for the force-pubsub crate.
//!
//! All errors originating from Pub/Sub API operations are represented by
//! [`PubSubError`]. A convenience [`Result`] alias is provided for use
//! throughout the crate.

use thiserror::Error;

/// All errors from the force-pubsub crate.
#[derive(Debug, Error)]
pub enum PubSubError {
    /// gRPC transport or protocol error.
    #[error("gRPC transport error: {0}")]
    Transport(#[from] tonic::Status),

    /// Avro encoding or decoding failure.
    #[error("Avro error: {0}")]
    Avro(String),

    /// Schema not found for the given ID.
    #[error("schema not found: {schema_id}")]
    SchemaNotFound {
        /// The schema ID that could not be resolved.
        schema_id: String,
    },

    /// Authentication or token error from the force crate.
    #[error("auth error: {0}")]
    Auth(#[from] force::error::ForceError),

    /// Reconnection to the subscribe stream was exhausted.
    #[error("reconnect failed after {attempts} attempt(s): {last_error}")]
    ReconnectFailed {
        /// Number of reconnection attempts made.
        attempts: u32,
        /// The error returned by the last attempt.
        last_error: Box<Self>,
    },

    /// gRPC channel setup failed.
    #[error("failed to connect to Pub/Sub endpoint: {0}")]
    Connect(#[from] tonic::transport::Error),

    /// Invalid configuration.
    #[error("invalid configuration: {0}")]
    Config(String),
}

/// Convenience Result alias for Pub/Sub operations.
pub type Result<T, E = PubSubError> = std::result::Result<T, E>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_not_found_display() {
        let err = PubSubError::SchemaNotFound {
            schema_id: "abc123".to_string(),
        };
        assert_eq!(err.to_string(), "schema not found: abc123");
    }

    #[test]
    fn test_config_error_display() {
        let err = PubSubError::Config("batch_size must be > 0".to_string());
        assert_eq!(
            err.to_string(),
            "invalid configuration: batch_size must be > 0"
        );
    }

    #[test]
    fn test_avro_error_display() {
        let err = PubSubError::Avro("unexpected end of buffer".to_string());
        assert_eq!(err.to_string(), "Avro error: unexpected end of buffer");
    }

    #[test]
    fn test_reconnect_failed_display() {
        let inner = Box::new(PubSubError::Config("test".to_string()));
        let err = PubSubError::ReconnectFailed {
            attempts: 3,
            last_error: inner,
        };
        assert!(err.to_string().contains("3 attempt(s)"));
    }

    #[test]
    fn test_from_tonic_status() {
        let status = tonic::Status::not_found("topic not found");
        let err = PubSubError::from(status);
        assert!(matches!(err, PubSubError::Transport(_)));
    }
}
