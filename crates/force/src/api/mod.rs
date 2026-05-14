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

pub mod query_analyzer;
pub(crate) mod query_stream;
pub(crate) mod rest_operation;
pub(crate) mod soql;

pub use query_analyzer::{QueryHealth, QueryHealthAnalyzer};
pub use rest_operation::RestOperation;
pub use soql::{SoqlQueryBuilder, escape_soql};

pub(crate) mod builder_unwrap;
pub(crate) mod path_utils;

#[cfg(any(feature = "composite", feature = "composite_graph"))]
pub(crate) mod url_encoded_writer;
