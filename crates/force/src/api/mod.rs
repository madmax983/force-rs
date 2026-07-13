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

#[cfg(feature = "models")]
pub mod models;

#[cfg(feature = "agent_api")]
pub mod agent_api;

#[cfg(feature = "account_engagement")]
pub mod account_engagement;

#[cfg(feature = "analytics")]
pub mod analytics;

#[cfg(feature = "soap")]
pub mod soap;

pub(crate) mod query_stream;
pub(crate) mod rest_operation;
pub(crate) mod soql;

pub use rest_operation::RestOperation;
pub use soql::{SoqlQueryBuilder, escape_soql};

pub(crate) mod builder_unwrap;
pub(crate) mod path_utils;

#[cfg(any(feature = "composite", feature = "composite_graph"))]
pub(crate) mod url_encoded_writer;
