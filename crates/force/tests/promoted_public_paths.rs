#![allow(clippy::expect_used)]
//! Compile-surface checks for promoted public module paths.
//!
//! This test exists to pin the public API layout for promoted preview utilities.

#![allow(unused_imports)]

#[cfg(feature = "rest")]
use force::api::rest::{
    ExplainResponse, InsightSeverity, PlanNote, QueryInsight, QueryInsights, QueryPlan,
    analyze_query_plan,
};

#[cfg(feature = "schema")]
use force::schema::{
    DataDictionary, FieldChange, FieldUsage, FieldUsageScanner, LintResult, LintRule, LintSeverity,
    MissingCustomSuffixRule, SchemaDiffResult, SchemaGraph, SchemaInsights, SchemaLinter,
    TooManyFieldsRule, analyze_schema, compare_schemas, generate_changelog, generate_ddl,
    generate_json_schema, generate_openapi_schema, generate_rust_struct,
    generate_typescript_interface, generate_visualizer_report, write_changelog, write_ddl,
    write_graphql_schema, write_openapi_schema, write_pydantic_model, write_typescript_interface,
};

#[cfg(feature = "data_utility")]
use force::data::{DataSeeder, generate_mock_record};

#[cfg(feature = "composite")]
use force::api::composite::{BatchOp, BatchStats, QueryBatch, SoqlMassOp};

#[test]
fn promoted_public_paths_compile() {}
