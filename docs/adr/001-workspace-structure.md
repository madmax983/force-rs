# ADR-001: Workspace Structure and Module Organization

**Status:** Accepted
**Date:** 2026-02-07
**Deciders:** Mark M, team-lead, auth-specialist
**Context:** Initial project setup for force-rs Salesforce API client

## Context and Problem Statement

We need to establish the foundational structure for force-rs, a production-grade Salesforce Platform API client. The structure must support:
- Multiple potential crates (client library, CLI tools, examples)
- Feature-gated compilation for 15+ API surfaces
- Clear separation of concerns
- Workspace-wide linting and dependency management
- Easy navigation and maintenance

How should we structure the project to support these requirements while following Rust best practices?

## Decision Drivers

- **Zero-cost abstractions** - Only compile what's used via feature flags
- **Maintainability** - Clear module boundaries and responsibilities
- **Developer experience** - Easy to find code, understand architecture
- **Workspace benefits** - Shared lints, versions, and dependencies
- **Mark's standards** - Follows established patterns from aletheiadb, thorp

## Considered Options

### Option 1: Flat Single-Crate Structure
```
force-rs/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── auth.rs
    ├── client.rs
    └── api/
```

**Pros:**
- Simple initial setup
- No workspace complexity
- Single compilation unit

**Cons:**
- Hard to add companion crates later (CLI, examples)
- No shared workspace configuration
- Becomes unwieldy as project grows

### Option 2: Workspace with Single Library Crate (CHOSEN)
```
force-rs/
├── Cargo.toml           # Workspace root
└── crates/
    └── force/           # Main library
        ├── Cargo.toml
        └── src/
```

**Pros:**
- Ready for future crates (force-cli, force-examples)
- Workspace-level lints and dependencies
- Clear separation of concerns
- Standard Rust workspace pattern
- Easy to add integration tests at workspace level

**Cons:**
- Slightly more complex initial setup
- Nested directory structure

### Option 3: Multiple Crates from Day One
```
force-rs/
├── Cargo.toml
└── crates/
    ├── force/           # Library
    ├── force-cli/       # CLI tool
    └── force-core/      # Shared types
```

**Pros:**
- Splits concerns early
- Forces good API boundaries

**Cons:**
- Over-engineering for initial phase
- Adds compilation complexity
- Premature abstraction

## Decision Outcome

**Chosen: Option 2 - Workspace with Single Library Crate**

We will use a Cargo workspace with a single `force` library crate in `crates/force/`. This provides room to grow while keeping initial complexity manageable.

### Workspace Configuration

**Root `Cargo.toml`:**
```toml
[workspace]
resolver = "2"
members = ["crates/force"]

[workspace.package]
version = "0.1.0"
edition = "2024"
authors = ["Mark M"]
license = "MIT OR Apache-2.0"

[workspace.lints.rust]
unsafe_code = "forbid"
missing_docs = "warn"

[workspace.lints.clippy]
pedantic = { level = "warn", priority = -1 }
nursery = { level = "warn", priority = -1 }
unwrap_used = "warn"
expect_used = "warn"

[workspace.dependencies]
# Centralized dependency versions
```

**Library `crates/force/Cargo.toml`:**
```toml
[package]
name = "force"
version.workspace = true
edition.workspace = true

[lints]
workspace = true

[dependencies]
tokio = { workspace = true }
# Feature-gated dependencies
jsonwebtoken = { workspace = true, optional = true }

[features]
default = ["rest"]
rest = []
jwt = ["dep:jsonwebtoken"]
full = ["rest", "bulk", "jwt"]
```

### Module Organization Within `crates/force/src/`

```
src/
├── lib.rs                 # Public API surface, re-exports
├── types/                 # Core domain types
│   ├── mod.rs
│   ├── salesforce_id.rs  # SalesforceId newtype
│   └── api_version.rs    # ApiVersion newtype
├── error/                 # Error hierarchy
│   ├── mod.rs
│   ├── auth_error.rs
│   ├── http_error.rs
│   └── api_error.rs
├── auth/                  # Authentication layer
│   ├── mod.rs
│   ├── traits.rs         # Authenticator trait
│   ├── token.rs          # AccessToken, TokenManager
│   ├── client_credentials.rs
│   └── jwt_bearer.rs     # Feature-gated: jwt
├── http/                  # HTTP client with middleware
│   ├── mod.rs
│   ├── client.rs
│   ├── retry.rs
│   ├── rate_limit.rs
│   └── middleware.rs
├── client/                # High-level ForceClient
│   ├── mod.rs
│   ├── force_client.rs
│   ├── builder.rs
│   └── config.rs
└── api/                   # API surface implementations
    ├── rest/             # Feature: rest (default)
    │   ├── mod.rs
    │   ├── query.rs
    │   └── crud.rs
    ├── bulk/             # Feature: bulk
    └── composite/        # Feature: composite
```

### Dependency Management Strategy

**Core dependencies** (always compiled):
- `tokio` - Async runtime
- `reqwest` - HTTP client with rustls
- `serde`/`serde_json` - Serialization
- `thiserror` - Error types
- `secrecy` - Credential protection
- `chrono` - Time handling

**Feature-gated dependencies** (optional):
- `jsonwebtoken` - JWT auth flow (feature: `jwt`)
- `csv` - Bulk API (feature: `bulk`)
- `tonic`/`prost` - Pub/Sub gRPC (feature: `pub_sub`)
- `quick-xml` - SOAP API (feature: `soap`)

**Dev dependencies**:
- `tokio-test` - Async test utilities
- `wiremock` - HTTP mocking (feature: `mock`)
- `anyhow` - Test error handling

## Consequences

### Positive

✅ **Workspace benefits** - Centralized linting, versions, dependency resolution
✅ **Future-proof** - Easy to add force-cli, force-examples, force-macros later
✅ **Clear boundaries** - Module structure maps to architectural layers
✅ **Feature isolation** - API surfaces and auth flows cleanly separated
✅ **Standards compliance** - Follows Mark's patterns from other projects

### Negative

⚠️ **Initial complexity** - Workspace setup more involved than flat structure
⚠️ **Nested paths** - `crates/force/src/...` adds directory depth
⚠️ **Documentation** - Need clear CLAUDE.md to explain structure

### Neutral

ℹ️ **Single library** - Start with one crate, expand as needed
ℹ️ **Module hierarchy** - Can refactor as patterns emerge during TDD

## Validation

This decision will be validated through:
1. Successful implementation of core types with TDD
2. Feature flag compilation works correctly
3. Easy navigation and code discovery
4. CI pipeline setup (fmt, clippy, test)
5. Team feedback during implementation

## Related Decisions

- [ADR-002](002-authentication-strategy.md) - Auth module design depends on this structure
- [ADR-003](003-error-handling.md) - Error module follows this organization
- [ADR-004](004-feature-gates.md) - Feature flags enabled by workspace structure

## References

- [Cargo Workspaces](https://doc.rust-lang.org/book/ch14-03-cargo-workspaces.html)
- [Mark's CLAUDE.md Standards](C:\Users\markm\.claude\CLAUDE.md)
- Existing projects: aletheiadb, thorp (workspace patterns)
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
