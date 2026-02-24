# force-rs

[![Build Status](https://img.shields.io/github/actions/workflow/status/markm/force-rs/ci.yml?branch=main)](https://github.com/markm/force-rs/actions)
[![Crates.io](https://img.shields.io/crates/v/force.svg)](https://crates.io/crates/force)
[![Documentation](https://docs.rs/force/badge.svg)](https://docs.rs/force)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)

**A canonical Salesforce Platform API client for Rust** — built with production-grade safety, performance, and developer ergonomics.

force-rs provides idiomatic Rust bindings to the Salesforce Platform APIs, enabling you to build high-performance integrations, data pipelines, and automation tools. With comprehensive REST and Bulk API 2.0 support, typestate-enforced workflows, and memory-efficient streaming, force-rs is designed for real-world enterprise workloads.

## Features

### REST API
- **CRUD Operations** - Create, read, update, delete, and upsert records with full type safety
- **SOQL Queries** - Execute typed queries with automatic pagination and streaming results
- **SOSL Search** - Full-text search across multiple objects with builder pattern
- **Metadata Access** - Describe objects, fields, and org limits programmatically
- **Relationship Support** - Query parent-child and lookup relationships seamlessly

### Bulk API 2.0
- **Typestate Safety** - Compile-time guarantees for job lifecycle (Open → Upload → InProgress → Complete)
- **Ingest Jobs** - Insert, update, upsert, and delete millions of records efficiently
- **Query Jobs** - Execute bulk queries with streaming CSV results
- **Memory Efficient** - Stream large datasets without loading entire payloads into RAM
- **Error Handling** - Comprehensive job monitoring and failure analysis

### Core Features
- **Multiple Auth Flows** - JWT bearer, OAuth 2.0 client credentials
- **Feature-Gated** - Enable only the APIs you need for minimal binary size
- **Async/Await** - Built on Tokio for high-concurrency workloads
- **Type-Safe Errors** - Structured error types with context for debugging
- **Production Ready** - 339 tests, zero clippy warnings, comprehensive examples

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

## Advanced Examples

### Bulk Insert with Typestate Safety

The Bulk API uses typestate patterns to enforce correct job lifecycle at compile time:

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
    let auth = ClientCredentials::new_production(
        "client-id",
        "client-secret",
    );
    let client = ForceClientBuilder::new().authenticate(auth).build().await?;

    let accounts = vec![
        Account { name: "Acme Corp".into(), industry: "Technology".into() },
        Account { name: "Global Ltd".into(), industry: "Manufacturing".into() },
    ];

    // Convenience method handles: create job → upload CSV → close → poll
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

#[derive(Debug, Deserialize)]
struct Contact {
    #[serde(rename = "Id")]
    id: String,
    #[serde(rename = "Email")]
    email: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let auth = ClientCredentials::new_production(
        "client-id",
        "client-secret",
    );
    let client = ForceClientBuilder::new().authenticate(auth).build().await?;

    // Create bulk query job and stream results
    let mut stream = client.bulk()
        .query::<Contact>(
            "SELECT Id, Email FROM Contact WHERE Email != null"
        )
        .await?;

    let mut count = 0;
    while let Some(contact) = stream.next().await? {
        println!("Processing: {} ({})", contact.id, contact.email.unwrap_or_default());
        count += 1;
    }

    println!("Streamed {} contacts", count);
    Ok(())
}
```

## More Examples

The [`examples/`](crates/force/examples) directory contains comprehensive demonstrations:

| Example | Description |
|---------|-------------|
| [`basic_crud.rs`](crates/force/examples/basic_crud.rs) | Complete CRUD lifecycle (create, read, update, delete) |
| [`soql_query.rs`](crates/force/examples/soql_query.rs) | Typed queries with pagination and relationships |
| [`dynamic_query.rs`](crates/force/examples/dynamic_query.rs) | Dynamic queries without predefined types |
| [`search.rs`](crates/force/examples/search.rs) | SOSL full-text search with builder pattern |
| [`describe.rs`](crates/force/examples/describe.rs) | Object and field metadata introspection |
| [`org_limits.rs`](crates/force/examples/org_limits.rs) | API limits and usage monitoring |
| [`bulk_insert.rs`](crates/force/examples/bulk_insert.rs) | Bulk insert with job monitoring |
| [`bulk_query.rs`](crates/force/examples/bulk_query.rs) | Bulk query with streaming results |
| [`bulk_update.rs`](crates/force/examples/bulk_update.rs) | Bulk update operations |
| [`bulk_delete.rs`](crates/force/examples/bulk_delete.rs) | Bulk delete with error handling |

Run any example with:

```bash
cargo run --example soql_query
cargo run --example bulk_insert --features bulk
```

## Features Reference

force-rs uses feature flags to minimize dependencies and binary size:

| Feature | Description |
|---------|-------------|
| `rest` | REST API (CRUD, SOQL, SOSL, describe, limits) — enabled by default |
| `bulk` | Bulk API 2.0 (ingest and query jobs) |
| `jwt` | JWT bearer token authentication |
| `composite` | Composite API (batch requests) |
| `tooling` | Tooling API (Apex, metadata) |
| `metadata` | Metadata API (deployment, retrieval) |
| `graphql` | GraphQL API support |
| `pub_sub` | Pub/Sub API (Change Data Capture, Platform Events) |
| `streaming` | Streaming API (Push Topics, Generic Streaming) |
| `analytics` | Analytics REST API |
| `connect` | Chatter REST API |
| `soap` | SOAP API support |
| `mock` | Wiremock utilities for testing |
| `full` | All APIs except experimental (`pub_sub`, `streaming`, `soap`) |
| `all` | Everything including experimental features |

**Recommendation:** Start with `default` features, then add `bulk` and `jwt` as needed.

## Roadmap

force-rs v0.1.0 provides production-ready REST and Bulk API support. Future releases will add:

- **v0.2.0** - Composite API for batch operations
- **v0.3.0** - Tooling API for Apex and metadata operations
- **v0.4.0** - Metadata API for deployment and retrieval
- **v0.5.0** - Pub/Sub API for Change Data Capture and Platform Events
- **v1.0.0** - Complete Salesforce Platform API coverage with stability guarantees


## Testing

force-rs has comprehensive test coverage (339 tests) using wiremock for HTTP mocking:

```bash
# Run all tests
cargo test --all-features

# Run with logging
RUST_LOG=debug cargo test --all-features

# Run specific test
cargo test --test rest_crud_tests --features rest
```

## Contributing

Contributions are welcome! force-rs follows strict TDD discipline and quality standards:

- **Test-Driven Development** - All features require failing tests first (RED → GREEN → REFACTOR)
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

**Built with ❤️ by the force-rs contributors** | [Documentation](https://docs.rs/force) | [Examples](crates/force/examples) | [Issues](https://github.com/markm/force-rs/issues)
