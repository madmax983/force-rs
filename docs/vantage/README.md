# Vantage Specs

Design backlog and product-spec notes for incubating `force-rs` capabilities. These documents were moved out of the repo root so release-facing docs stay focused on shipped crates and active project documentation.

## Streaming and Integration

- [`bulk-pk-chunking.md`](bulk-pk-chunking.md): Bulk API PK Chunking Support.
- [`streaming-api.md`](streaming-api.md): CometD/Bayeux streaming API support.

> Apex REST, Consent & Portability, Data Cloud, SOAP, and Data Faker shipped and
> moved to [`docs/guide/surfaces/`](../guide/surfaces/README.md); their vantage
> specs were removed as superseded. Data Seeder shipped its generate-and-insert
> path, but its spec is narrowed and kept — see
> [`data-seeder.md`](data-seeder.md) — for the one acceptance criterion that
> didn't ship: structured per-record failure reporting.
>
> The `force-sync` and `force-pubsub` crates, the `force::schema` module (Avro,
> BigQuery, DBML, GraphQL SDL, JSON Schema, OpenAPI, Postman, Prisma, Protobuf,
> Pydantic, Rust struct, SQL DDL, TypeScript, Zod generators, plus the schema
> analyzer, changelog, linter, and visualizer), `force::api::rest::analyze_query_plan`,
> and `force::api::SoqlQueryBuilder` all shipped the same way (see
> [CHANGELOG.md](../../CHANGELOG.md)); their vantage specs described these as
> unbuilt gaps and were removed as superseded. None of these surfaces has a
> [`docs/guide/surfaces/`](../guide/surfaces/README.md) page yet — tracked as a
> separate coverage gap, not a vantage spec.

## Metadata and Tooling

- [`metadata-api.md`](metadata-api.md): Metadata API deployment and retrieval operations.

## Content and Files

- [`salesforce-files-api.md`](salesforce-files-api.md): streaming (async reader/writer) upload/download for Salesforce Files, on top of the shipped in-memory `files` handler.

## Data Utilities

- [`data-seeder.md`](data-seeder.md): structured per-record failure reporting for the shipped `DataSeeder` (narrowed — see the status banner in that file).

## Authentication

- [`saml-bearer-flow.md`](saml-bearer-flow.md): OAuth 2.0 SAML Bearer Assertion Flow for Salesforce authentication.
