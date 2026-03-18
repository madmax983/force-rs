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

pub mod soql;

pub(crate) mod path_utils;

#[cfg(any(feature = "composite", feature = "composite_graph"))]
pub(crate) mod url_encoded_writer;
