//! REST API handler for Salesforce.
//!
//! This module provides the `RestHandler` which serves as the foundation for all
//! REST API operations including CRUD, queries, and metadata operations.

pub mod crud;
pub mod describe;
#[cfg(feature = "nova")]
pub mod explain;
pub mod handler;
pub mod limits;
pub mod query;
pub mod query_stream;
pub mod search;
pub mod soql;

pub use handler::RestHandler;
pub use query_stream::QueryStream;
pub use soql::{SoqlQueryBuilder, escape_soql};
