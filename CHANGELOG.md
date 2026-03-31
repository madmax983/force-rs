# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-03-30

### Added

#### Workspace
- Initial crates.io release of the `force-rs` workspace with `force`, `force-pubsub`, and `force-sync`.

#### `force`
- Feature-gated Salesforce client surfaces for REST, Bulk API 2.0, Composite, Tooling, UI API, GraphQL, Data Cloud, Apex REST, Consent and Portability, and CPQ.
- Authentication flows for client credentials, JWT bearer, and username/password, plus automatic token refresh through the shared session stack.
- Strongly typed core primitives including `SalesforceId`, `ApiVersion`, `QueryLocator`, `DynamicSObject`, and compile-time bulk job state handling.
- Release-ready utility modules under stable public paths: `force::schema`, `force::data`, `force::api::rest::analyze_query_plan`, and `force::api::composite::{QueryBatch, SoqlMassOp}`.

#### `force-pubsub`
- gRPC Pub/Sub client for Salesforce CDC, Platform Events, and custom channels.
- Replay-aware subscribe flows, schema-aware publish support, and concurrent Avro schema caching.

#### `force-sync`
- Postgres-first bidirectional sync engine built on top of `force` and `force-pubsub`.
- Explicit capture, planning, apply, reconcile, and recovery entry points with embedded migrations and outbox-backed sync tables.

### Changed

- Removed the pre-release `nova` feature and retired the old `force::experimental` namespace before the `0.1.0` cut.
- Promoted stable utilities into feature-gated public modules so the published API surface matches the release documentation.

### Documentation

- Refreshed workspace docs and examples for the final published crate layout.
- Moved Vantage specs out of the repository root into `docs/vantage/` to keep release-facing docs focused on shipped crates.

[0.1.0]: https://github.com/madmax983/force-rs/releases/tag/v0.1.0
