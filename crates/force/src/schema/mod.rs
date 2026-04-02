//! Schema inspection, analysis, and code-generation utilities.

pub(crate) mod data_dictionary;
pub(crate) mod json_schema;
pub(crate) mod mock_data_generator;
pub(crate) mod openapi_generator;
pub(crate) mod postman_generator;
pub(crate) mod pydantic_generator;
pub(crate) mod scanner;
pub(crate) mod schema_analyzer;
pub(crate) mod schema_changelog;
pub(crate) mod schema_diff;
pub(crate) mod schema_graph;
pub(crate) mod schema_linter;
pub(crate) mod schema_visualizer;
pub(crate) mod sql_exporter;
#[allow(missing_docs)]
pub(crate) mod type_generator;
pub(crate) mod typescript_generator;

pub use data_dictionary::DataDictionary;
pub use json_schema::generate_json_schema;
pub use mock_data_generator::generate_mock_data;
pub use openapi_generator::generate_openapi_schema;
pub use postman_generator::generate_postman_collection;
pub use pydantic_generator::generate_pydantic_model;
pub use scanner::{FieldUsage, FieldUsageScanner};
pub use schema_analyzer::{SchemaInsights, analyze_schema};
pub use schema_changelog::generate_changelog;
pub use schema_diff::{FieldChange, SchemaDiffResult, compare_schemas};
pub use schema_graph::SchemaGraph;
pub use schema_linter::{
    LintResult, LintRule, LintSeverity, MissingCustomSuffixRule, SchemaLinter, TooManyFieldsRule,
};
pub use schema_visualizer::generate_visualizer_report;
pub use sql_exporter::generate_ddl;
pub use type_generator::generate_rust_struct;
pub use typescript_generator::generate_typescript_interface;
