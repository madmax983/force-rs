//! Salesforce Pub/Sub API client for Rust.
//!
//! This crate provides a gRPC client for the Salesforce Pub/Sub API,
//! supporting subscribe, publish, and schema operations.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::missing_errors_doc)]

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
