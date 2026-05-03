//! Schema inspection, analysis, and code-generation utilities.

#[cfg(feature = "schema")]
pub(crate) mod avro_generator;
#[cfg(feature = "schema")]
pub(crate) mod bigquery_generator;
pub(crate) mod data_anonymizer;
pub(crate) mod data_dictionary;
#[cfg(feature = "schema")]
pub(crate) mod dbml_generator;
pub(crate) mod graphql_generator;
pub(crate) mod json_schema;
pub(crate) mod mock_data_generator;
pub(crate) mod openapi_generator;
pub(crate) mod postman_generator;
#[cfg(feature = "schema")]
pub(crate) mod prisma_generator;
#[cfg(feature = "schema")]
pub(crate) mod protobuf_generator;
pub(crate) mod pydantic_generator;
pub(crate) mod scanner;
pub(crate) mod schema_analyzer;
pub(crate) mod schema_changelog;
pub(crate) mod schema_diff;
pub(crate) mod schema_graph;
pub(crate) mod schema_limits;
pub(crate) mod schema_linter;
pub(crate) mod schema_visualizer;
pub(crate) mod sql_exporter;
#[allow(missing_docs)]
pub(crate) mod type_generator;
pub(crate) mod typescript_generator;
pub(crate) mod zod_generator;

pub(crate) fn cmp_field_names(a: &str, b: &str) -> std::cmp::Ordering {
    match (a == "Id", b == "Id") {
        (true, true) => std::cmp::Ordering::Equal,
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        (false, false) => a.cmp(b),
    }
}

#[cfg(feature = "schema")]
pub use avro_generator::{generate_avro_schema, write_avro_schema};
#[cfg(feature = "schema")]
pub use bigquery_generator::generate_bigquery_schema;
#[cfg(feature = "schema")]
pub use data_anonymizer::DataAnonymizer;
pub use data_dictionary::DataDictionary;
#[cfg(feature = "schema")]
pub use dbml_generator::{generate_dbml, write_dbml};
pub use graphql_generator::{generate_graphql_schema, write_graphql_schema};
pub use json_schema::generate_json_schema;
pub use mock_data_generator::generate_mock_data;
pub use openapi_generator::{generate_openapi_schema, write_openapi_schema};
pub use postman_generator::generate_postman_collection;
#[cfg(feature = "schema")]
pub use prisma_generator::{generate_prisma_model, write_prisma_model};
#[cfg(feature = "schema")]
pub use protobuf_generator::{generate_protobuf_schema, write_protobuf_schema};
pub use pydantic_generator::{generate_pydantic_model, write_pydantic_model};
pub use scanner::{FieldUsage, FieldUsageScanner};
pub use schema_analyzer::{SchemaInsights, analyze_schema};
pub use schema_changelog::{generate_changelog, write_changelog};
pub use schema_diff::{FieldChange, SchemaDiffResult, compare_schemas};
pub use schema_graph::SchemaGraph;
pub use schema_limits::{SchemaLimitInsights, analyze_schema_limits};
pub use schema_linter::{
    LintResult, LintRule, LintSeverity, MissingCustomSuffixRule, SchemaLinter, TooManyFieldsRule,
};
pub use schema_visualizer::generate_visualizer_report;
pub use sql_exporter::{generate_ddl, write_ddl};
pub use type_generator::generate_rust_struct;
pub use typescript_generator::{generate_typescript_interface, write_typescript_interface};
pub use zod_generator::{generate_zod_schema, write_zod_schema};

pub(crate) mod llm_context_generator;
pub use llm_context_generator::{LlmContextOptions, generate_llm_context};

#[cfg(feature = "schema")]
pub(crate) mod dbt_generator;
#[cfg(feature = "schema")]
pub use dbt_generator::{generate_dbt_source_yml, generate_dbt_staging_model};
