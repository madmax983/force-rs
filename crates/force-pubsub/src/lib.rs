//! Salesforce Pub/Sub API client for Rust.
//!
//! This crate provides a gRPC client for the Salesforce Pub/Sub API,
//! supporting subscribe, publish, and schema operations.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::missing_errors_doc)]
// tonic::Status is 176 bytes; suppressed crate-wide rather than boxing
// through every From impl and call site.
#![allow(clippy::result_large_err)]
#![allow(dead_code)] // Phase in progress — handler fields used by subscribe/publish tasks

pub mod codec;
pub mod config;
pub mod error;
pub mod handler;
pub mod types;

pub(crate) mod publisher;
pub mod schema_cache;
pub(crate) mod subscriber;

pub use config::{BackoffConfig, PubSubConfig, ReconnectPolicy, ReplayPreset};
pub use error::{PubSubError, Result};
pub use handler::{PubSubHandler, SchemaInfo, TopicInfo};
pub use schema_cache::SchemaCache;
pub use types::{EventMessage, PubSubEvent, PublishResponse, PublishResult, ReplayId};

/// Generated protobuf types from the Salesforce Pub/Sub API proto.
pub mod proto {
    /// Generated types from the `eventbus.v1` protobuf package.
    pub mod eventbus_v1 {
        // Generated code does not carry doc comments or follow all clippy rules.
        #![allow(missing_docs)]
        #![allow(clippy::derive_partial_eq_without_eq)]
        #![allow(clippy::must_use_candidate)]
        #![allow(clippy::missing_const_for_fn)]
        #![allow(clippy::default_trait_access)]
        #![allow(clippy::too_many_lines)]
        tonic::include_proto!("eventbus.v1");
    }
}
