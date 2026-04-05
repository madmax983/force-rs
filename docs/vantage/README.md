# Vantage Specs

Design backlog and product-spec notes for incubating `force-rs` capabilities. These documents were moved out of the repo root so release-facing docs stay focused on shipped crates and active project documentation.

## Streaming and Integration

- [`apex-rest-api.md`](apex-rest-api.md): native Apex REST handler for custom Salesforce endpoints.
- [`consent-portability-api.md`](consent-portability-api.md): Consent and Portability API support.
- [`force-sync-engine.md`](force-sync-engine.md): Postgres-first bidirectional sync engine.
- [`pub-sub-api.md`](pub-sub-api.md): gRPC event streaming via Salesforce Pub/Sub.

## Schema and Codegen

- [`data-dictionary-generator.md`](data-dictionary-generator.md): metadata-to-documentation generation.
- [`field-usage-scanner.md`](field-usage-scanner.md): schema usage analysis for low-value fields.
- [`json-schema-generator.md`](json-schema-generator.md): JSON Schema generation from org metadata.
- [`openapi-generator.md`](openapi-generator.md): OpenAPI generation for downstream consumers.
- [`postman-collection-generator.md`](postman-collection-generator.md): Postman collection generation for API testing.
- [`rust-struct-generator.md`](rust-struct-generator.md): Rust type generation from Salesforce schema.
- [`schema-analyzer.md`](schema-analyzer.md): schema quality and risk analysis.
- [`schema-changelog-generator.md`](schema-changelog-generator.md): schema diff and release changelog generation.
- [`schema-linter.md`](schema-linter.md): linting rules for schema hygiene.
- [`schema-visualizer.md`](schema-visualizer.md): visual schema graphing.
- [`sql-ddl-exporter.md`](sql-ddl-exporter.md): SQL DDL export for warehouse and database workflows.
- [`typescript-interface-generator.md`](typescript-interface-generator.md): TypeScript interface generation.
- [`zod-schema-generator.md`](zod-schema-generator.md): Zod runtime validation schema generation.

## Metadata and Tooling

- [`metadata-api.md`](metadata-api.md): Metadata API deployment and retrieval operations.

## Content and Files

- [`salesforce-files-api.md`](salesforce-files-api.md): streaming multipart file uploads and downloads.

## Data Utilities

- [`data-faker.md`](data-faker.md): synthetic Salesforce-shaped data generation.
- [`data-seeder.md`](data-seeder.md): repeatable fixture and seed workflows.
