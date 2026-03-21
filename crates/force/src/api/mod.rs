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

pub mod rest_operation;
pub mod soql;

pub(crate) mod path_utils;

#[cfg(any(feature = "composite", feature = "composite_graph"))]
pub(crate) mod url_encoded_writer;
