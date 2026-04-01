//! Salesforce API handlers.
//!
//! This module provides access to various Salesforce APIs through feature-gated
//! submodules.

#[cfg(feature = "rest")]
pub mod rest;

#[cfg(feature = "bulk")]
pub mod bulk;

#[cfg(feature = "composite")]
pub mod composite;

#[cfg(feature = "tooling")]
pub mod tooling;

#[cfg(feature = "ui")]
pub mod ui;

#[cfg(feature = "graphql")]
pub mod graphql;

#[cfg(feature = "data_cloud")]
pub mod data_cloud;

#[cfg(feature = "apex_rest")]
pub mod apex_rest;

#[cfg(feature = "cpq")]
pub mod cpq;

#[cfg(feature = "consent")]
pub mod consent;

pub(crate) mod query_stream;
pub mod rest_operation;
pub mod soql;

pub(crate) mod path_utils;

#[cfg(any(feature = "composite", feature = "composite_graph"))]
pub(crate) mod url_encoded_writer;

/// Extension trait to unwrap builder `try_` methods or panic with a formatted error message.
pub trait BuilderUnwrap<T> {
    /// Unwraps a builder result, panicking with a standardized "Invalid input in {context}: {error}" message if it fails.
    fn unwrap_or_builder_panic(self, context: &str) -> T;
}

impl<T> BuilderUnwrap<T> for Result<T, crate::error::ForceError> {
    fn unwrap_or_builder_panic(self, context: &str) -> T {
        self.unwrap_or_else(|e| panic!("Invalid input in {}: {}", context, e))
    }
}
