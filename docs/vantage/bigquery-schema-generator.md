# BigQuery Schema Generator

## Overview
A utility to generate Google BigQuery table schema arrays from Salesforce `SObjectDescribe` metadata.

## User Story
As a Data Engineer, I want to convert Salesforce object definitions into BigQuery schemas, so that I can automatically provision destination tables for my data lake without manual mapping.

## The "So What?"
**What business problem does this solve?**
Data pipelines syncing Salesforce to BigQuery currently require manual schema mapping, which is error-prone, tedious, and breaks when Salesforce fields are added or changed. By generating schemas dynamically from Salesforce metadata, we eliminate pipeline maintenance overhead, reduce integration friction, and accelerate time-to-insight for analytics teams.

## Metric Definition
- **Success =** 100% of standard Salesforce data types (`FieldType`) map without error to BigQuery data types, with zero schema-validation rejections when submitted to the BigQuery API.

## Gap Analysis
Currently, users of `force-rs` must manually translate `SObjectDescribe` fields into custom JSON or rely on brittle third-party Python tools outside the Rust ecosystem. The `force::schema` module provides generators for protobuf and pydantic, but lacks native BigQuery output. Adding this bridges the gap between Salesforce data retrieval and modern cloud data warehouses.

## Acceptance Criteria
- Must map Salesforce data types (e.g., `string`, `int`, `boolean`, `datetime`) to BigQuery native types (`STRING`, `INT64`, `BOOL`, `TIMESTAMP`).
- Must respect the `nillable` attribute (mapping to BigQuery `NULLABLE` vs `REQUIRED`).
- Must output valid JSON schema arrays compatible with the BigQuery API.

## Out of Scope
- Actually creating the tables in BigQuery via API.
- Generating schemas for nested fields or complex relationships not present in base SObjectDescribe.
