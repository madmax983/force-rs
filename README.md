# force-rs

[![Build Status](https://img.shields.io/github/actions/workflow/status/madmax983/force-rs/ci.yml?branch=main)](https://github.com/madmax983/force-rs/actions)
[![Crates.io](https://img.shields.io/crates/v/force.svg)](https://crates.io/crates/force)
[![Documentation](https://docs.rs/force/badge.svg)](https://docs.rs/force)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)

**A canonical Salesforce Platform API client for Rust** — built with production-grade safety, performance, and developer ergonomics.

force-rs provides idiomatic Rust bindings to the Salesforce Platform APIs, enabling you to build high-performance integrations, data pipelines, and automation tools. With comprehensive coverage of 7 API surfaces, compile-time safe workflows, and memory-efficient streaming, force-rs is designed for real-world enterprise workloads.

The workspace also includes [`force-sync`](crates/force-sync), a Postgres-first bidirectional sync engine built on top of `force` and `force-pubsub`.

## Features

### REST API
- **CRUD Operations** - Create, read, update, delete, and upsert records with full type safety
- **SOQL Queries** - Execute typed queries with automatic pagination and streaming results
- **SOSL Search** - Full-text search across multiple objects with builder pattern
- **Metadata Access** - Describe objects, fields, and org limits programmatically
- **Relationship Support** - Query parent-child and lookup relationships seamlessly

### Bulk API 2.0
- **Compile-Time Safety** - Strict guarantees for job lifecycle (Open -> Upload -> InProgress -> Complete)
- **Ingest Jobs** - Insert, update, upsert, and delete millions of records efficiently
- **Query Jobs** - Execute bulk queries with streaming CSV results
- **Memory Efficient** - Stream large datasets without loading entire payloads into RAM
- **Error Handling** - Comprehensive job monitoring and failure analysis

### Composite API
- **Batch Requests** - Combine up to 25 subrequests in a single HTTP call
- **Graph Requests** - Up to 500 nodes with dependency ordering (feature: `composite_graph`)
- **Reduced API Consumption** - Minimize round trips and stay within governor limits

### Tooling API
- **Apex Management** - Query and manage Apex classes, triggers, and components
- **Execute Anonymous** - Run Apex code on the fly with full result inspection
- **Test Execution** - Run Apex tests synchronously or asynchronously
- **Code Completions** - IDE-style completions for Apex and Visualforce

### UI API
- **Layout-Aware Records** - Get presentation-ready data with display values and field visibility
- **Object Metadata** - Retrieve field info, picklist values, and record type mappings
- **List Views** - Access list view definitions, columns, and paginated records
- **Lookups & Favorites** - Type-ahead search and user favorite management

### GraphQL API
- **Unified Queries** - Request specific fields and nested relationships in a single call
- **Typed Results** - Deserialize into custom Rust structs or use dynamic `Value`
- **Variables & Operations** - Parameterized queries with named operations
- **Partial Success Handling** - Inspect both data and errors when both are present

### Core Features
- **Multiple Auth Flows** - JWT bearer, OAuth 2.0 client credentials
- **Feature-Gated** - Enable only the APIs you need for minimal binary size
- **Async/Await** - Built on Tokio for high-concurrency workloads
- **Type-Safe Errors** - Structured error types with context for debugging
- **Production Ready** - 870+ tests, zero clippy warnings, comprehensive examples

## Installation

Add force-rs to your `Cargo.toml`:

```toml
[dependencies]
force = "0.1"
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
anyhow = "1.0"

# Or enable specific features:
force = { version = "0.1", features = ["rest", "bulk", "jwt"] }
```

## Quick Start

Here's a minimal example using OAuth 2.0 client credentials to query Salesforce:

```rust
use force::auth::ClientCredentials;
use force::client::ForceClientBuilder;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Account {
    #[serde(rename = "Id")]
    id: String,
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Industry")]
    industry: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Authenticate with OAuth 2.0 client credentials
    // For Sandbox, use: ClientCredentials::new_sandbox("client-id", "client-secret")
    let auth = ClientCredentials::new_production(
        "your-client-id",
        "your-client-secret",
    );

    let client = ForceClientBuilder::new()
        .authenticate(auth)
        .build()
        .await?;

    // Execute typed SOQL query
    let soql = "SELECT Id, Name, Industry FROM Account WHERE Industry = 'Technology' LIMIT 10";
    let result = client.rest().query::<Account>(soql).await?;

    // Process results
    for account in result.records {
        println!("{}: {} ({})",
            account.id,
            account.name,
            account.industry.unwrap_or_default()
        );
    }

    Ok(())
}
```

> **Note:** For Sandbox environments, use `ClientCredentials::new_sandbox("client-id", "client-secret")` instead of `new_production`.

## Advanced Examples

### GraphQL Query

Query specific fields and nested relationships in a single request:

```rust
// Requires the "graphql" feature: force = { version = "0.1", features = ["graphql"] }
use force::api::graphql::GraphqlRequest;
use force::auth::ClientCredentials;
use force::client::ForceClientBuilder;
use serde_json::json;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let auth = ClientCredentials::new_production("client-id", "client-secret");
    let client = ForceClientBuilder::new().authenticate(auth).build().await?;
    let gql = client.graphql();

    // Simple raw query
    let data = gql.query_raw(
        r#"{ uiapi { query { Account(first: 5) {
            edges { node { Id Name { value } } }
            totalCount
        } } } }"#,
        None,
    ).await?;

    println!("Total: {}", data["uiapi"]["query"]["Account"]["totalCount"]);

    // Query with variables
    let req = GraphqlRequest::new("query($limit: Int) { uiapi { query { Account(first: $limit) { edges { node { Id } } } } } }")
        .with_variables(json!({"limit": 10}))
        .with_operation_name("GetAccounts");
    let data: serde_json::Value = gql.query(&req).await?;

    Ok(())
}
```

### Bulk Insert with Compile-Time Safety

The Bulk API uses Rust's type system to enforce the correct job lifecycle at compile time:

```rust
// Requires the "bulk" feature: force = { version = "0.1", features = ["bulk"] }
use force::client::ForceClientBuilder;
use force::auth::ClientCredentials;
use serde::Serialize;

#[derive(Serialize)]
struct Account {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Industry")]
    industry: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // For Sandbox, use: ClientCredentials::new_sandbox("client-id", "client-secret")
    let auth = ClientCredentials::new_production(
        "client-id",
        "client-secret",
    );
    let client = ForceClientBuilder::new().authenticate(auth).build().await?;

    let accounts = vec![
        Account { name: "Acme Corp".into(), industry: "Technology".into() },
        Account { name: "Global Ltd".into(), industry: "Manufacturing".into() },
    ];

    // Convenience method handles: create job -> upload CSV -> close -> poll
    let job_info = client.bulk().insert("Account", &accounts).await?;

    println!("Processed: {}, Failed: {}",
        job_info.number_records_processed.unwrap_or(0),
        job_info.number_records_failed.unwrap_or(0)
    );

    Ok(())
}
```

### Memory-Efficient Bulk Query

Stream millions of records without loading the entire dataset into memory:

```rust
// Requires the "bulk" feature: force = { version = "0.1", features = ["bulk"] }
use force::client::ForceClientBuilder;
use force::auth::ClientCredentials;
use serde::Deserialize;
// Requires `futures` crate
use futures::StreamExt;

#[derive(Debug, Deserialize)]
struct Contact {
    #[serde(rename = "Id")]
    id: String,
    #[serde(rename = "Email")]
    email: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // For Sandbox, use: ClientCredentials::new_sandbox("client-id", "client-secret")
    let auth = ClientCredentials::new_production(
        "client-id",
        "client-secret",
    );
    let client = ForceClientBuilder::new().authenticate(auth).build().await?;

    // Create bulk query job and stream results
    let stream = client.bulk()
        .query::<Contact>(
            "SELECT Id, Email FROM Contact WHERE Email != null"
        )
        .await?;
    let stream = stream.into_stream();
    let mut stream = std::pin::pin!(stream);

    let mut count = 0;
    while let Some(contact_result) = stream.next().await {
        let contact = contact_result?;
        println!("Processing: {} ({})", contact.id, contact.email.unwrap_or_default());
        count += 1;
    }

    println!("Streamed {} contacts", count);
    Ok(())
}
```

## More Examples

The [`examples/`](crates/force/examples) directory contains comprehensive demonstrations:

| Example | Feature | Description |
|---------|---------|-------------|
| [`basic_crud.rs`](crates/force/examples/basic_crud.rs) | `rest` | Complete CRUD lifecycle (create, read, update, delete) |
| [`soql_query.rs`](crates/force/examples/soql_query.rs) | `rest` | Typed queries with pagination and relationships |
| [`dynamic_query.rs`](crates/force/examples/dynamic_query.rs) | `rest` | Dynamic queries without predefined types |
| [`search.rs`](crates/force/examples/search.rs) | `rest` | SOSL full-text search with builder pattern |
| [`describe.rs`](crates/force/examples/describe.rs) | `rest` | Object and field metadata introspection |
| [`org_limits.rs`](crates/force/examples/org_limits.rs) | `rest` | API limits and usage monitoring |
| [`bulk_insert.rs`](crates/force/examples/bulk_insert.rs) | `bulk` | Bulk insert with job monitoring |
| [`bulk_query.rs`](crates/force/examples/bulk_query.rs) | `bulk` | Bulk query with streaming results |
| [`bulk_update.rs`](crates/force/examples/bulk_update.rs) | `bulk` | Bulk update operations |
| [`bulk_delete.rs`](crates/force/examples/bulk_delete.rs) | `bulk` | Bulk delete with error handling |
| [`tooling.rs`](crates/force/examples/tooling.rs) | `tooling` | Apex classes, anonymous execution, completions, tests |
| [`ui_api.rs`](crates/force/examples/ui_api.rs) | `ui` | Layout-aware records, object info, list views, favorites |
| [`graphql.rs`](crates/force/examples/graphql.rs) | `graphql` | GraphQL queries, typed results, variables, error handling |
| [`soql_mass_op.rs`](crates/force/examples/soql_mass_op.rs) | `composite` | Composite batch operations |
| [`query_plan.rs`](crates/force/examples/query_plan.rs) | `rest` | SOQL query plan inspection |

Run any example with:

```bash
cargo run --example soql_query
cargo run --example bulk_insert --features bulk
cargo run --example graphql --features graphql
```

## Features Reference

force-rs uses feature flags to minimize dependencies and binary size:

| Feature | Description | Status |
|---------|-------------|--------|
| `rest` | REST API (CRUD, SOQL, SOSL, describe, limits) | Default |
| `bulk` | Bulk API 2.0 (ingest and query jobs) | Stable |
| `composite` | Composite API (batch requests) | Stable |
| `composite_graph` | Composite Graph API (dependency-ordered nodes) | Stable |
| `tooling` | Tooling API (Apex, execute anonymous, tests, completions) | Stable |
| `ui` | UI API (layout-aware records, object info, list views, favorites) | Stable |
| `graphql` | GraphQL API (queries, mutations, variables) | Stable |
| `jwt` | JWT bearer token authentication | Stable |
| `schema` | Schema analysis, scanning, and code generation utilities | Preview |
| `data_utility` | Mock-data generation and Salesforce seeding helpers | Preview |
| `mock` | Wiremock utilities for testing | Stable |
| `full` | All stable APIs (`rest` + `bulk` + `composite` + `tooling` + `ui` + `graphql` + `jwt`) | Meta |
| `all` | Everything including preview features | Meta |

**Recommendation:** Start with `default` features, then add `bulk` and `jwt` as needed.

## Preview Features

The `force` crate includes preview features for early adopters. These capabilities live in their real modules and remain feature-gated while the API settles before a future stabilization pass.

### Query Plan API

> Available with the default `rest` feature.

The Query Plan API allows you to inspect the performance cost of a SOQL query before executing it. This is useful for identifying inefficient queries (e.g., table scans) in CI/CD pipelines.

```toml
[dependencies]
force = "0.1"
```

```rust
// Available with the default "rest" feature: force = "0.1"
use force::client::ForceClientBuilder;
use force::auth::ClientCredentials;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let auth = ClientCredentials::new_production("client-id", "client-secret");
    let client = ForceClientBuilder::new().authenticate(auth).build().await?;

    let soql = "SELECT Id FROM Account WHERE Name LIKE 'A%'";
    let explanation = client.rest().explain(soql).await?;

    for plan in explanation.plans {
        println!("Plan: {}, Cost: {}", plan.leading_operation_type, plan.relative_cost);
        for note in plan.notes {
            println!("  Note: {}", note.description);
        }
    }
    Ok(())
}
```

### Preview Utility Modules

Preview utilities now live in the modules that own them:

- **`force::api::composite::{QueryBatch, SoqlMassOp}`**: batch-aware helpers built on the Composite API.
- **`force::api::rest::analyze_query_plan`**: turns `explain()` responses into actionable warnings.
- **`force::schema`**: schema scanning, diffing, visualization, DDL export, and code generation helpers.
- **`force::data`**: mock-record generation and bulk seeding helpers.

## Architecture

force-rs is built around a handler pattern where each API surface gets its own feature-gated handler type:

```
ForceClient<A>
  |-- .rest()       -> RestHandler<A>       (feature: rest)
  |-- .bulk()       -> BulkHandler<A>       (feature: bulk)
  |-- .composite()  -> CompositeHandler<A>  (feature: composite)
  |-- .tooling()    -> ToolingHandler<A>    (feature: tooling)
  |-- .ui()         -> UiHandler<A>         (feature: ui)
  |-- .graphql()    -> GraphqlHandler<A>    (feature: graphql)
```

All handlers share a common `Session<A>` (via `Arc`) containing the HTTP client, token manager, and configuration. This ensures zero-cost handler creation and shared authentication state.

For the sync layer, see [`crates/force-sync`](crates/force-sync) and its design notes in [`docs/adr/026-force-sync-crate.md`](docs/adr/026-force-sync-crate.md).

Architectural decisions are documented in [`docs/adr/`](docs/adr/):

| ADR | Decision |
|-----|----------|
| [001](docs/adr/001-workspace-structure.md) | Workspace structure and module organization |
| [002](docs/adr/002-authentication-strategy.md) | Authentication trait design and flow support |
| [003](docs/adr/003-error-handling.md) | Error hierarchy with thiserror |
| [004](docs/adr/004-feature-gates.md) | Feature flag strategy for API surfaces |
| [005](docs/adr/005-compile-time-auth-safety.md) | Compile-time auth safety with phantom types |
| [006](docs/adr/006-handler-pattern.md) | Handler pattern for API organization |
| [007](docs/adr/007-rest-api-design.md) | REST API design decisions |
| [019](docs/adr/019-tooling-api-design.md) | RestOperation trait and Tooling API |
| [020](docs/adr/020-ui-api-design.md) | UI API handler design |
| [021](docs/adr/021-graphql-api-design.md) | GraphQL API error handling strategy |

## Testing

force-rs has comprehensive test coverage (870+ tests) using wiremock for HTTP mocking:

```bash
# Run all tests
cargo test --all-features

# Run with logging
RUST_LOG=debug cargo test --all-features

# Run specific API surface tests
cargo test --features graphql -- graphql
cargo test --features bulk -- bulk
```

Nightly live-contract tests (ignored by default in local runs) are available in CI and can be run manually with org credentials.

## Enterprise DX and Governance

### Runbooks

- [Auth Credential Rotation](docs/runbooks/auth-credential-rotation.md)
- [Rate-Limit Incident Response](docs/runbooks/rate-limit-incident.md)
- [Retry and Polling Tuning](docs/runbooks/retry-tuning.md)
- [Salesforce API Version Upgrade](docs/runbooks/salesforce-api-version-upgrade.md)
- [Documentation Index](docs/README.md)

### API Guarantees

The crate-level compatibility and feature-flag guarantees are documented in:

- [API Stability and SemVer Policy](docs/governance/api-stability-policy.md)

### Incubation Specs

- [Vantage Specs Index](docs/vantage/README.md)

### CI Lanes

- Fast unit/lint/format gates for PR velocity
- Full test lanes for broader confidence
- Nightly live-contract workflow for real Salesforce contract validation

## Contributing

Contributions are welcome! force-rs follows strict TDD discipline and quality standards:

- **Test-Driven Development** - All features require failing tests first (RED -> GREEN -> REFACTOR)
- **Code Quality** - `cargo fmt` and `cargo clippy -- -D warnings` must pass
- **Documentation** - All public APIs require doc comments with examples
- **Architecture** - ADRs (Architecture Decision Records) for significant changes

See [`CONTRIBUTING.md`](CONTRIBUTING.md) for detailed guidelines.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.

---

**Built by the force-rs contributors** | [Documentation](https://docs.rs/force) | [Examples](crates/force/examples) | [Issues](https://github.com/markm/force-rs/issues)
