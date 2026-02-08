# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-02-08

### Added

#### Core Infrastructure
- `ForceClient<A>` with compile-time authentication safety via generic authenticator trait
- `TokenManager` with automatic refresh and `Arc<RwLock>` state management
- Comprehensive error hierarchy using `thiserror` (HTTP, authentication, API, serialization errors)
- Feature-gated architecture for optional API surfaces
- Workspace structure with `force` library crate

#### Authentication
- Client Credentials OAuth 2.0 flow (`ClientCredentials`)
- JWT Bearer flow for server-to-server integration (`JwtBearer`)
- SAML Bearer flow for enterprise SSO scenarios (`SamlBearer`)
- Automatic token refresh on expiration (401 responses)
- Secure handling of credentials using `secrecy` crate

#### REST API (feature: `rest`)
- **CRUD Operations**: Create, read, update, delete, upsert with typed and dynamic support
- **Query API**: SOQL execution with `QueryStream` for automatic pagination
- **Search API**: SOSL queries with builder pattern for multi-object searches
- **Describe API**: Schema introspection for SObjects, fields, and picklists
- **Limits API**: Organization limits monitoring
- Full support for typed SObjects via `serde::Deserialize` and `DynamicSObject` for runtime fields

#### Bulk API 2.0 (feature: `bulk`)
- **Ingest Jobs**: Typestate pattern for compile-time job lifecycle safety (`Open` → `UploadComplete` → `InProgress` → `JobComplete`)
- **Bulk Query**: High-volume SOQL queries with CSV result streaming
- **CSV Utilities**: Memory-efficient chunk reading and writing
- **Job Management**: Create, upload, abort, delete operations
- **Status Polling**: Automatic polling with configurable intervals

#### Type System
- `SalesforceId` newtype with 15/18-character validation
- `ApiVersion` with safe default (v62.0)
- `QueryLocator` for transparent pagination
- Strongly-typed job states for bulk operations

#### Quality & Testing
- **339 passing tests** across all modules with zero warnings
- Test-driven development (TDD) with RED-GREEN-REFACTOR discipline
- `wiremock` integration for HTTP mocking in tests
- Comprehensive error handling with no `unwrap()` in production code
- Full `cargo clippy` compliance with pedantic/nursery lints

#### Documentation
- 8 Architecture Decision Records (ADRs) documenting design choices
- Doc comments with examples on all public APIs
- 6 runnable examples: basic CRUD, queries, search, describe, limits, bulk operations
- README with quick-start guide and feature gate documentation

### Technical Highlights
- **Zero-cost abstractions**: Generic authenticator with no runtime overhead
- **Memory efficiency**: Streaming APIs for query results and CSV data
- **Compile-time safety**: Typestate pattern prevents invalid bulk job transitions
- **Production-ready**: Retry logic, rate limiting, structured errors

[0.1.0]: https://github.com/markm/force-rs/releases/tag/v0.1.0
