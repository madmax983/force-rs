# Vantage Specs

Design backlog and product-spec notes for incubating `force-rs` capabilities. These documents were moved out of the repo root so release-facing docs stay focused on shipped crates and active project documentation.

## Streaming and Integration

- [`bulk-pk-chunking.md`](bulk-pk-chunking.md): Bulk API PK Chunking Support.
- [`force-sync-engine.md`](force-sync-engine.md): Postgres-first bidirectional sync engine.
- [`pub-sub-api.md`](pub-sub-api.md): gRPC event streaming via Salesforce Pub/Sub.
- [`streaming-api.md`](streaming-api.md): CometD/Bayeux streaming API support.

> Apex REST, Consent & Portability, Data Cloud, and SOAP shipped (v0.1.0 and
> v0.4.0 respectively) and moved to
> [`docs/guide/surfaces/`](../guide/surfaces/README.md); their vantage specs
> were removed as superseded. Data Faker shipped the same way and its spec
> was removed too. Data Seeder shipped its generate-and-insert path, but its
> spec is narrowed and kept — see
> [`data-seeder.md`](data-seeder.md) — for the one acceptance criterion that
> didn't ship: structured per-record failure reporting.

## Schema and Codegen

- [`avro-schema-generator.md`](avro-schema-generator.md): Apache Avro schema generation.
- [`data-dictionary-generator.md`](data-dictionary-generator.md): metadata-to-documentation generation.
- [`field-usage-scanner.md`](field-usage-scanner.md): schema usage analysis for low-value fields.
- [`graphql-schema-generator.md`](graphql-schema-generator.md): GraphQL Schema Definition Language (SDL) generation.
- [`json-schema-generator.md`](json-schema-generator.md): JSON Schema generation from org metadata.
- [`openapi-generator.md`](openapi-generator.md): OpenAPI generation for downstream consumers.
- [`postman-collection-generator.md`](postman-collection-generator.md): Postman collection generation for API testing.
- [`protobuf-schema-generator.md`](protobuf-schema-generator.md): Protobuf (proto3) schema generation for gRPC services.
- [`pydantic-model-generator.md`](pydantic-model-generator.md): Python Pydantic model generation for data pipelines.
- [`rust-struct-generator.md`](rust-struct-generator.md): Rust type generation from Salesforce schema.
- [`schema-analyzer.md`](schema-analyzer.md): schema quality and risk analysis.
- [`schema-changelog-generator.md`](schema-changelog-generator.md): schema diff and release changelog generation.
- [`schema-linter.md`](schema-linter.md): linting rules for schema hygiene.
- [`schema-visualizer.md`](schema-visualizer.md): visual schema graphing.
- [`sql-ddl-exporter.md`](sql-ddl-exporter.md): SQL DDL export for warehouse and database workflows.
- [`typescript-interface-generator.md`](typescript-interface-generator.md): TypeScript interface generation.
- [`zod-schema-generator.md`](zod-schema-generator.md): TypeScript Zod schema generation for runtime validation.
- [`bigquery-schema-generator.md`](bigquery-schema-generator.md): Google BigQuery table schema generation.
- [`prisma-schema-generator.md`](prisma-schema-generator.md): Prisma ORM schema generation for Node.js/TypeScript applications.

## Metadata and Tooling

- [`metadata-api.md`](metadata-api.md): Metadata API deployment and retrieval operations.
- [`query-plan-analyzer.md`](query-plan-analyzer.md): SOQL query performance analyzer and cost threshold validation.

## Content and Files

- [`salesforce-files-api.md`](salesforce-files-api.md): streaming (async reader/writer) upload/download for Salesforce Files, on top of the shipped in-memory `files` handler.

## Data Utilities

- [`soql-query-builder.md`](soql-query-builder.md): SOQL query builder for type-safe query construction.
- [`data-seeder.md`](data-seeder.md): structured per-record failure reporting for the shipped `DataSeeder` (narrowed — see the status banner in that file).

## Authentication

- [`saml-bearer-flow.md`](saml-bearer-flow.md): OAuth 2.0 SAML Bearer Assertion Flow for Salesforce authentication.
