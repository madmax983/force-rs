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

#[cfg(feature = "schema")]
pub(crate) mod schema_graph;

#[cfg(feature = "schema")]
pub(crate) mod data_dictionary;

#[cfg(feature = "schema")]
#[allow(missing_docs)]
pub(crate) mod type_generator;

#[cfg(feature = "schema")]
pub(crate) mod schema_diff;

#[cfg(feature = "data_utility")]
pub(crate) mod data_faker;

#[cfg(feature = "schema")]
pub(crate) mod schema_analyzer;
#[cfg(feature = "schema")]
pub(crate) mod schema_visualizer;

#[cfg(feature = "data_utility")]
pub(crate) mod sql_exporter;
#[cfg(feature = "data_utility")]
pub use sql_exporter::generate_ddl;

#[cfg(feature = "composite")]
pub(crate) mod soql_mass_op;

#[cfg(feature = "composite")]
pub use query_batch::{BatchOp, BatchStats, QueryBatch};

#[cfg(feature = "schema")]
pub use schema_graph::SchemaGraph;

#[cfg(feature = "schema")]
pub use data_dictionary::DataDictionary;

#[cfg(feature = "schema")]
pub use type_generator::generate_rust_struct;

#[cfg(feature = "schema")]
pub use schema_diff::{FieldChange, SchemaDiffResult, compare_schemas};

#[cfg(feature = "data_utility")]
pub use data_faker::generate_mock_record;

#[cfg(feature = "schema")]
pub use schema_analyzer::{SchemaInsights, analyze_schema};
#[cfg(feature = "schema")]
pub use schema_visualizer::generate_visualizer_report;

#[cfg(feature = "composite")]
pub use soql_mass_op::SoqlMassOp;

#[cfg(feature = "schema")]
pub(crate) mod schema_changelog;

#[cfg(feature = "schema")]
pub use schema_changelog::generate_changelog;
#[cfg(feature = "schema")]
pub(crate) mod typescript_generator;
#[cfg(feature = "schema")]
pub use typescript_generator::generate_typescript_interface;
#[cfg(feature = "schema")]
pub(crate) mod openapi_generator;
#[cfg(feature = "schema")]
pub use openapi_generator::generate_openapi_schema;

#[cfg(feature = "schema")]
pub(crate) mod json_schema;
#[cfg(feature = "schema")]
pub use json_schema::generate_json_schema;

#[cfg(feature = "schema")]
pub(crate) mod schema_linter;
#[cfg(feature = "schema")]
pub use schema_linter::{
    LintResult, LintRule, LintSeverity, MissingCustomSuffixRule, SchemaLinter, TooManyFieldsRule,
};

#[cfg(feature = "nova")]
pub(crate) mod query_plan_analyzer;
#[cfg(feature = "nova")]
pub use query_plan_analyzer::{InsightSeverity, QueryInsight, QueryInsights, analyze_query_plan};

#[cfg(feature = "schema")]
pub(crate) mod dependency_mapper;

#[cfg(feature = "schema")]
pub use dependency_mapper::{DependencyNode, SObjectDependencyMapper};
