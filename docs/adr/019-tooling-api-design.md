# ADR-019: RestOperation Trait and Tooling API Design

**Status:** Accepted
**Date:** 2026-03-19
**Deciders:** Mark

## Context

The Tooling API provides the same CRUD, Query, and Describe operations as the
standard REST API. The only difference is that all requests are routed under the
`/tooling/` URL prefix:

| Operation | REST API | Tooling API |
|-----------|----------|-------------|
| Create | `POST /sobjects/Account` | `POST /tooling/sobjects/ApexClass` |
| Query | `GET /query?q=...` | `GET /tooling/query?q=...` |
| Describe | `GET /sobjects/Account/describe` | `GET /tooling/sobjects/ApexClass/describe` |

Duplicating the full CRUD/Query/Describe implementations in both `RestHandler`
and `ToolingHandler` would violate DRY and create a maintenance burden where
every bug fix or enhancement must be applied in two places.

Additionally, the `tooling` feature flag must be independent of `rest`. Users
who only need the Tooling API should not be forced to compile the REST API
module, and vice versa.

The Tooling API also exposes development-specific endpoints that have no REST
API equivalent:

- `executeAnonymous` -- compile and execute arbitrary Apex code
- `runTestsSynchronous` / `runTestsAsynchronous` -- execute Apex unit tests
- `completions` -- code completion suggestions for Apex and Visualforce

## Decision

### 1. Extract `RestOperation` trait with default implementations

A new trait `RestOperation<A: Authenticator>` provides default implementations
for all shared operations. Implementors supply only two methods:

```rust
pub trait RestOperation<A: Authenticator> {
    fn session(&self) -> &Arc<Session<A>>;
    fn path_prefix(&self) -> &str;
    // ... default methods: create, get, update, delete, upsert, query, describe, etc.
}
```

`RestHandler` returns `""` (empty prefix), routing to the standard REST API.
`ToolingHandler` returns `"tooling"`, routing to the Tooling API.

A `resolve_api_path()` helper prepends the prefix to relative paths:

```
resolve_api_path("sobjects/Account")
  -> "sobjects/Account"         (RestHandler, prefix = "")
  -> "tooling/sobjects/Account" (ToolingHandler, prefix = "tooling")
```

### 2. Tooling-specific endpoints as direct methods

Endpoints unique to the Tooling API are implemented as methods directly on
`ToolingHandler` in their respective submodules:

- `execute_anonymous.rs` -- `ToolingHandler::execute_anonymous(code)`
- `run_tests.rs` -- `ToolingHandler::run_tests(request)` and `run_tests_async(request)`
- `completions.rs` -- `ToolingHandler::completions(type, query)`

### 3. Independent feature flag

The `tooling` feature flag is independent of `rest`:

```toml
[features]
rest = []
tooling = []
full = ["rest", "bulk", "composite", "tooling", "jwt"]
```

The `rest_operation` module is always compiled (it contains the shared trait),
while `api::rest` and `api::tooling` are each gated behind their own feature.

### 4. Trait import required for shared operations

Consumers must import `RestOperation` to access CRUD, Query, and Describe
methods on either handler:

```rust
use force::api::rest_operation::RestOperation;

// Now .create(), .query(), .describe(), etc. are available on both:
client.rest().create("Account", &data).await?;
client.tooling().create("ApexClass", &data).await?;
```

## Consequences

### Positive

- **DRY.** CRUD, Query, and Describe logic is implemented once in the trait's
  default methods. Bug fixes and improvements apply to both handlers
  automatically.
- **Minimal handler boilerplate.** A new handler needs only ~20 lines: a struct,
  a `new()` constructor, and two trait method implementations.
- **Feature isolation.** `tooling` compiles without `rest` and vice versa.
  Users pay compile-time cost only for the APIs they use.
- **Extensibility.** Future handlers (e.g., Metadata API, Analytics API) can
  implement `RestOperation` to inherit the same shared operations.
- **Clear separation.** Tooling-specific endpoints live in their own modules,
  not polluting the shared trait.

### Negative

- **Breaking change for consumers.** Code that previously called `.create()`
  or `.query()` directly on `RestHandler` must now import the `RestOperation`
  trait. This is a one-line addition per file.
- **Trait method dispatch.** Default trait methods cannot be overridden per-handler
  without explicit specialization. If a handler needs divergent behavior for a
  shared operation, it must override the default method. This has not been needed
  so far.
- **`async_fn_in_trait` usage.** The trait uses `#[allow(async_fn_in_trait)]`
  since the trait is used internally and does not need `Send` bounds. This is
  stable as of Rust 1.75 and appropriate for our use case.
