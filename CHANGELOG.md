# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

Durable sync on autumn-harvest. See [ADR-036](docs/adr/036-durable-sync-on-autumn-harvest.md).

### Breaking

#### `force-pubsub`
- `PubSubEvent::KeepAlive` is now `KeepAlive { latest_replay_id: Option<ReplayId> }`. It carries the latest replay ID that Salesforce sends with each keepalive, so a client can resume a quiet channel with no gap. Match it as `PubSubEvent::KeepAlive { .. }`.

### Added

#### `force-sync`
- Durable Salesforce Change Data Capture: `DurableSync::with_cdc(source, streams)` follows each `CdcStream` in one long-running `force_sync_cdc_stream` workflow (`force-sync-cdc-{stream}`), kept running by the `force_sync_cdc_supervisor` schedule. A restart resumes from the checkpoint with no lost or duplicate event. New config: `cdc_window`, `cdc_supervise_interval`, `cdc_windows_per_run`, `cdc_error_backoff`.
- `capture_cdc_window` and `SyncEngine::run_cdc_window`: one bounded CDC capture window from the checkpoint. An event that can never be captured, such as one without the external ID field, goes to `sync_dead_letter` and the checkpoint moves past it.
- CDC capture stores the latest replay ID of each keepalive as the checkpoint (`capture_cdc_window` and `capture_stream`). The first window of a `CdcStart::Latest` stream with no checkpoint stays open up to `FIRST_POSITION_WAIT` until it has a position, so a quiet stream never skips the events between windows.
- `CdcSource` trait, implemented for `force_pubsub::PubSubHandler`, with `CdcStream`, `CdcStart`, `CdcWindow`, `CdcWindowReport`, `CdcWindowEnd`, and `CdcEventStream`.
- New opt-in `durable` feature: runs capture, apply and reconcile as [autumn-harvest](https://crates.io/crates/autumn-harvest) 0.7 workflows. Needs Rust 1.88 or later.
  - `durable::DurableSync` with `start()` (a standalone harvest worker and scheduler on the `force-sync` queue) and `register(HarvestBuilder)` (add durable sync to an existing harvest app).
  - `DurableSyncConfig`: pump and reconcile intervals, capture and dispatch batch sizes, dispatch lease, Salesforce retry policy, lease owner.
  - One `force_sync_apply` workflow per `sync_task` row (`force-sync-apply-{task_id}`). Its Salesforce call is a recorded activity, so a restart after the call does not repeat it.
  - At most one apply in flight per record (harvest concurrency key = sync key, limit 1).
  - Transient Salesforce errors retry with the configured backoff. Exhausted retries fail the task with `Salesforce call retries exhausted: ...`.
  - `durable::migrate_harvest` and `durable::harvest_pool` helpers.
- `PgStore::apply_task_keys` returns the sync key of each apply task.
- `PgStore::lease_ready_or_expired_tasks` leases ready tasks and reclaims expired leases of the same owner. The durable dispatch uses it, so a crash mid-dispatch cannot strand a task.
- `ForceSyncError::Durable` variant.
- `examples/durable_sync.rs`.

### Changed

#### `force-sync`
- `SyncEngine::run_apply_once` now runs the same prepare / execute / finalize phases as the durable runtime. Task states and error messages are unchanged.
- The bulk lane picks REST up front when the linked Salesforce ID is invalid. It used to call Bulk and then REST.

## [0.5.0] - 2026-10-28

Winter '27 (API v68.0) adoption. See [ADR-035](docs/adr/035-winter-27-v68-adoption.md) and the
[upgrade guide](docs/guide/06-upgrading.md) for a fix for each breaking change.

### Breaking

#### `force`
- Default API version is now v68.0 (Winter '27). `MAX_TESTED` is v68.0.
- `JobOperation` has new variants `Refresh` and `ConsentImport`. A `match` without a `_` arm must add them.
- `AuthenticationError` has a new variant `UnsupportedTokenType`.
- `CreateJobRequest` no longer sends an empty `object`. `JobInfo.object` decodes a missing or `null` value as `""`.
- `GraphqlRequest` sends `operationName` (was `operation_name`, which the server ignored). Tests that assert the request body must change the key.
- A DPoP-bound token (`token_type: DPoP`) now fails with `AuthenticationError::UnsupportedTokenType` before any request, on every path (REST, SOAP, Data Cloud exchange, Pub/Sub).
- The SOAP handler returns `InvalidInput` when `api_version` is `latest`.
- `SmartIngest` returns `InvalidInput` for `JobOperation::Refresh`. It splits input into many jobs, and each refresh job replaces the data set.
- Agent API methods return `InvalidInput` for an agent or session ID with characters other than `[A-Za-z0-9_-]`.

### Added

#### `force`
- `ApiVersion::V68`, `release_name()`, `LATEST_ALIAS`, and `is_latest_alias()`. `ClientConfig.api_version` accepts `"latest"` for REST resources.
- REST: `versions()` and `highest_version()` for the Versions resource (`GET /services/data/`), with `ApiVersionInfo`.
- Bulk API 2.0: `CreateJobRequest::new`, `consent_import`, and `validate`.
- Tooling: `apex_compile_results()`, `symbols()` (Beta), `discover_tests()`, and `discover_tests_next()`, with a client-side `testLevel`/`showAllMethods` version check.
- GraphQL: `lookup_values()` for `uiapi.lookupValues`.
- SOQL: `SoqlQueryBuilder::try_where_formula()` for `FORMULA()` (Beta), with `FormulaOperator`, `ComparisonOperator`, and `FormulaLiteral` (also re-exported from `force::api::rest`).
- New `api_catalog` feature (in `full`): MCP server registry handler (`client.api_catalog()`).
- Agent API: `send_message_stream()` and `send_text_stream()` (Server-Sent Events, 1 MiB per event), `with_stream_timeout()`, `stream_timeout()`, and `DEFAULT_STREAM_TIMEOUT`.
- `AccessToken::is_dpop_bound()`.

### Changed

#### `force`
- A per-request timeout (for example the stream timeout) now also sets the executor's time limit for that request.
- `AgentMessage.result` and `cited_references` accept `null` or one object, not only an array.
- Composite treats a `latest/...` sub-request URL as already versioned.

### Fixed

#### `force`
- `ApiVersion` constant docs had release names one year late (for example v67.0 is Summer '26, not Summer '25).

### Security

#### workspace
- Patched `zstd-safe` 7.3.0 for [RUSTSEC-2026-0403](https://rustsec.org/advisories/RUSTSEC-2026-0403) (vendored backport of the 8.2.0 fix; see `vendor/zstd-safe-7.3.0/FORCE-RS-PATCH.md`).
- Patched `zstd` 0.13.3 for [RUSTSEC-2026-0391](https://rustsec.org/advisories/RUSTSEC-2026-0391) (vendored backport of the 0.14.3 fix; see `vendor/zstd-0.13.3/FORCE-RS-PATCH.md`).

### Documentation
- ADR-035, the Winter '27 adoption plan, the API Catalog surface page, and Winter '27 OAuth notes (refresh-token idle limit, rotation, External Client Apps, DPoP, instanced URLs, username-password retirement on 2027-02-20).

## [0.4.0] - 2026-07-15

This release adds five new Salesforce API surfaces and two new workspace crates, raises the workspace MSRV to Rust 1.92, and includes API-shape corrections driven by live-org contract testing (see Changed). See [docs/guide/06-upgrading.md](docs/guide/06-upgrading.md) for a before/after fix for each breaking change below.

### Added

#### `force`
- OAuth 2.0 Authorization Code + PKCE authentication flow (feature `auth_code`): PKCE helpers, authorize-URL builder, code-to-token exchange for public and confidential clients, refresh-token rotation, and token revocation.
- Agentforce Models API (feature `models`): text, chat, and embedding generation through the Einstein LLM gateway on `api.salesforce.com`.
- Agentforce Agent API (feature `agent_api`): headless agent sessions (start/send/end) on `api.salesforce.com`.
- Account Engagement (Pardot) API v5 (feature `account_engagement`): business-unit-scoped prospects, lists, campaigns, custom fields, forms, and emails on the `pi.pardot.com` host, with a generic escape hatch for un-modeled objects.
- Reports & Dashboards REST API (feature `analytics`): report runs (sync and async), instances, describe, and dashboard results/refresh/status.
- SOAP Partner API (feature `soap`): untyped generic CRUD, query, search, and describe over `/services/Soap/u/` with the OAuth token carried in `SessionHeader`, plus a serde-typed convenience layer.
- Tiered, env-gated live-contract test harness (ADR-033), verified against a real org.

#### `force-lake`
- New crate: one-way Salesforce to S3 Tables / Apache Iceberg analytics snapshot sink, with Describe-to-Iceberg/Arrow schema mapping, Arrow RecordBatch assembly, in-memory Parquet encoding, and a `LakeCatalog` trait with `MockCatalog` and `S3TablesCatalog` (iceberg-rust 0.9).

#### `force-marketingcloud`
- New crate: standalone Marketing Cloud Engagement REST API client, with installed-package server-to-server auth, a per-business-unit token cache, transactional messaging, Content Builder assets, contacts, data extensions, and journeys.

### Changed

#### workspace
- Raised the minimum supported Rust version (MSRV) to 1.92 (edition 2024).

#### `force`
- API-shape corrections (semver-breaking). These change public types, so downstream code may need to be recompiled or lightly updated, but the previous shapes could not successfully round-trip against a live org:
  - `OrgLimits` numeric fields are now `Option`-wrapped, because real org responses omit some limits and previously failed to deserialize.
  - `RecordDefaultsRepresentation.object_info` (a single value) was replaced by `object_infos` (a map), matching the actual UI API payload.
  - GraphQL `query_raw` now returns an error when the response carries GraphQL errors, instead of returning null-filled data.
- Files API now routes through the shared HTTP executor, so it participates in the 401 token-refresh and retry middleware; previously it bypassed the middleware stack.

### Fixed

#### `force`
- Composite Graph now supports sObject graph nodes with the full `/services/data` sub-request path and GraphQL Id leaf queries, and normalizes graph URLs; live-contract deserialization was made resilient to real-org response variation.

### Performance

#### `force`
- SOAP: borrow-based XML escaping and short-circuit fault parsing (fault-parse latency down ~99.4%), and dropped the intermediate typed `Value` round-trip in typed deserialization (down ~61.5%). See `docs/perf/2026-07-perf-sweep.md`.
- HTTP: borrow the request method and path when emitting telemetry.

#### `force-lake`
- Pre-sized Arrow builders and borrowed decimal text during RecordBatch assembly.

#### `force-pubsub`
- Skip the token and metadata fetch on a schema-cache hit.

### Documentation

- Added a 22-page operator's guide under `docs/guide/`.
- Added the July 2026 performance sweep report (`docs/perf/2026-07-perf-sweep.md`).
- Added ADRs for the new surfaces and crates: ADR-027 (Authorization Code + PKCE), ADR-028 (Agentforce), ADR-029 (Account Engagement), ADR-030 (force-lake), ADR-031 (Reports & Dashboards), ADR-032 (SOAP), ADR-033 (live-contract harness), ADR-034 (force-marketingcloud).

## [0.3.0] - 2026-07-11

### Added

#### `force`
- Added `ApiVersion` constants for Salesforce API versions v61.0 through v67.0.
- Added a `DataValidator` (feature `data_utility`) for schema-aware validation of record payloads.
- Added a `RelationalSeeder` (features `data_utility` + `composite_graph`) for generating relationally consistent seed data across related objects.

### Changed

#### `force`
- Bumped the default Salesforce API version from v60.0 to v67.0.
- Unified error handling into a single central error enum for a more consistent, exhaustive error surface.
- Consolidated upsert handling into the shared `RestOperation` trait so REST and Tooling operations share one implementation.
- Reduced allocations and improved throughput across REST query batching, Composite batch execution, search field handling, schema diffing, DBML/OpenAPI/LLM-context generation, and data masking.

#### `force-sync`
- Tightened public module boundaries so internal sync details stay behind stable facade APIs.

### Security

#### `force`
- Restored SSRF- and token-leak-safe resolution of `nextRecordsUrl`, preventing query pagination from being redirected to an attacker-controlled host or leaking the access token.
- Prevented URL-encoded path traversal in `validate_url_path`.
- Fixed an integer-overflow denial-of-service in API error parsing and added checked arithmetic to schema complexity scoring.
- Fixed an `AccessToken` expiration overflow, a TOCTOU race in `TokenManager`, and a hard-refresh path that could overwrite a newer token.
- Guarded Bulk API batch handling against limit and overflow panics.

### Documentation

- Added a Vantage spec for the Data Cloud API and refreshed ADRs and release-facing documentation.

## [0.2.0] - 2026-04-23

### Added

#### `force`
- Expanded schema and data tooling with generators for Avro, BigQuery, DBML, GraphQL, Postman, Prisma, Protobuf, Pydantic, and Zod, plus data archiving, masking, seeding, and mock-data workflows.
- Added Salesforce Files support, query-plan analysis, and stronger live contract coverage across REST, Bulk API 2.0, Composite, Data Cloud, and multiple OAuth flows.
- Added `SmartIngest` orchestration that can split large ingest streams across multiple Bulk API 2.0 jobs while returning aggregate job results.

#### `force-pubsub`
- Added live Salesforce Pub/Sub contract coverage for topic and schema lookup against a real org endpoint.
- Hardened gRPC connection setup for HTTPS Pub/Sub endpoints by enabling TLS automatically when required.

#### `force-sync`
- Expanded the Postgres-backed sync runtime, planner, apply, and recovery coverage with stronger integration tests and runtime behavior validation.

### Changed

- Normalized live-auth configuration so JWT, client-credentials, and username-password test inputs accept bare hosts, base URLs, or full Salesforce token endpoints and fail with clearer configuration errors.
- Fixed Composite and REST path handling so live contract tests exercise normalized absolute URLs instead of tripping over relative-path builder errors.
- Tightened public module boundaries across workspace crates so implementation details stay behind cleaner facade-style APIs.

### Security

- Hardened request and response handling against path traversal, SSRF, unbounded deserialization, memory exhaustion, integer overflow/underflow, and related Bulk API and HTTP edge cases.

### Documentation

- Added and refreshed Vantage specs, ADRs, and release-facing docs for the expanded API surface and release workflow.

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

[0.5.0]: https://github.com/madmax983/force-rs/releases/tag/v0.5.0
[0.4.0]: https://github.com/madmax983/force-rs/releases/tag/v0.4.0
[0.3.0]: https://github.com/madmax983/force-rs/releases/tag/v0.3.0
[0.2.0]: https://github.com/madmax983/force-rs/releases/tag/v0.2.0
[0.1.0]: https://github.com/madmax983/force-rs/releases/tag/v0.1.0
