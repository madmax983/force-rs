# 🔭 Vantage: Spec for dbt Model Generator

## 👤 User Story
As an Analytics Engineer, I want to automatically generate dbt (data build tool) source definitions and base staging models from Salesforce metadata, so that I can quickly onboard Salesforce data into my analytics pipeline without manually typing out boilerplate SQL and YAML.

## The "So What?"
**What business problem does this solve?**
When integrating Salesforce data into a modern data stack (like Snowflake or BigQuery), analytics teams use dbt to transform the raw data. Writing the initial `source.yml` definitions and the `stg_salesforce__object.sql` models for dozens of SObjects (many with 100+ fields) is incredibly tedious and error-prone. By generating these starter files automatically, we reduce the time-to-value for analytics projects and ensure field names are perfectly synced with the upstream CRM.

## Metric Definition
- **Success =** The generator produces valid `source.yml` syntax and a syntactically correct `SELECT` statement for a base staging model that passes `dbt compile` for any given SObject.

## Gap Analysis
While we have generators for BigQuery schemas and SQL DDL, these address the *loading* phase (EL). Analytics Engineers need tooling for the *transformation* phase (T). Currently, they must manually write dbt boilerplate. A native dbt generator bridges the gap between raw data ingestion and analytics-ready models.

## ✅ Acceptance Criteria
- Must generate a `source.yml` entry containing the table name and a list of all fields with descriptions mapped from Salesforce labels.
- Must generate a staging model (`.sql`) that selects all fields from the source table.
- Must gracefully handle standard and custom fields.

## 🚫 Out of Scope
- Complex dbt macros or tests (e.g., `not_null`, `unique`).
- Generating dimensional models (facts/dimensions). We only generate the base staging layer.
