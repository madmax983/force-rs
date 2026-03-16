//! Experimental features.
//!
//! # Deprecation
//!
//! `SmartIngest` has been promoted to `crate::api::bulk::smart_ingest`.

#[cfg(feature = "bulk")]
#[deprecated(since = "0.2.0", note = "Use `crate::api::bulk::smart_ingest` instead")]
pub use crate::api::bulk::SmartIngest;

#[cfg(feature = "rest")]
pub(crate) mod scanner;

#[cfg(feature = "composite")]
pub(crate) mod query_batch;

#[cfg(feature = "nova")]
pub(crate) mod schema_graph;

#[cfg(feature = "nova")]
pub(crate) mod data_dictionary;

#[cfg(feature = "nova")]
#[allow(missing_docs)]
pub(crate) mod type_generator;

#[cfg(feature = "nova")]
pub(crate) mod schema_diff;

#[cfg(feature = "nova")]
pub(crate) mod data_faker;

#[cfg(feature = "nova")]
pub(crate) mod schema_analyzer;
#[cfg(feature = "nova")]
pub(crate) mod schema_visualizer;

#[cfg(feature = "nova")]
pub(crate) mod sql_exporter;

#[cfg(all(feature = "nova", feature = "composite"))]
pub(crate) mod soql_mass_op;

#[cfg(feature = "composite")]
pub use query_batch::{BatchOp, BatchStats, QueryBatch};

#[cfg(feature = "nova")]
pub use schema_graph::SchemaGraph;

#[cfg(feature = "nova")]
pub use data_dictionary::DataDictionary;

#[cfg(feature = "nova")]
pub use type_generator::StructGenerator;

#[cfg(feature = "nova")]
pub use schema_diff::{FieldChange, SchemaDiff, SchemaDiffResult};

#[cfg(feature = "nova")]
pub use data_faker::DataFaker;

#[cfg(feature = "nova")]
pub use schema_analyzer::{SchemaAnalyzer, SchemaInsights};
#[cfg(feature = "nova")]
pub use schema_visualizer::SchemaVisualizer;

#[cfg(all(feature = "nova", feature = "composite"))]
pub use soql_mass_op::SoqlMassOp;
#[cfg(feature = "nova")]
pub use sql_exporter::SqlExporter;
