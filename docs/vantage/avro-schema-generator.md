# 🔭 Vantage: Spec for Avro Schema Generator

## Overview
A utility to generate Apache Avro schema definitions (`.avsc`) from Salesforce `SObjectDescribe` metadata.

## User Story
As a Data Engineer or Integration Architect, I want to automatically convert Salesforce object metadata into Apache Avro schemas, so that I can easily serialize Salesforce records for streaming platforms like Kafka or long-term storage in data lakes without manually writing and maintaining schemas.

## The "So What?"
**What business problem does this solve?**
Many modern event-driven architectures and data lakes (like Databricks, Snowflake, or Hadoop) standardize on Apache Avro because of its compact binary format and robust schema evolution capabilities. Currently, integrating Salesforce data into these ecosystems requires developers to manually map hundreds of Salesforce fields to Avro types. This manual mapping is fragile, time-consuming, and prone to breaking when Salesforce administrators add or change fields. By dynamically generating the Avro schemas directly from the Salesforce API, we eliminate maintenance friction, ensure 100% schema accuracy, and dramatically speed up the development of enterprise streaming and ETL pipelines.

## Metric Definition
- **Success =** 100% of generated Avro schemas pass validation by the official Apache Avro schema parser without errors. The tool should be able to generate a schema for an object with 500+ fields in less than 50 milliseconds.

## Gap Analysis
The `force::schema` module currently provides generation capabilities for BigQuery, JSON Schema, Protobuf, and various typed languages, but lacks support for Apache Avro. Given Avro's dominance in streaming (Kafka) and data lake storage, adding this generator bridges a critical gap for enterprise customers using `force-rs` in high-throughput data pipelines.

## Acceptance Criteria
- Must map Salesforce data types (e.g., `string`, `int`, `boolean`, `currency`, `double`) to corresponding native Avro types (`string`, `int`, `boolean`, `double`).
- Must correctly handle the Salesforce `nillable` attribute by wrapping the Avro type in a union with `"null"` (e.g., `["null", "string"]`).
- Must structure the output as a valid Avro Record type, including `type`, `name`, `namespace` (e.g., `com.salesforce.schema`), and `fields`.
- Must sort the generated fields alphabetically for deterministic output and easier diffing, with the exception of the `Id` field which should always appear first.

## Out of Scope
- Actually serializing the Salesforce record data into the binary Avro format (this generator is only responsible for the schema definition).
- Registering the generated schemas with an external Schema Registry (e.g., Confluent Schema Registry).
