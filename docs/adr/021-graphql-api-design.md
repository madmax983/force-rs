# ADR-021: GraphQL API Design

**Date:** 2026-03-20
**Status:** Accepted
**Feature flag:** `graphql`

## Context

The Salesforce GraphQL API (`POST /services/data/vXX.0/graphql`) provides a unified query interface using the GraphQL query language. It differs from both the REST API and the UI API in several important ways:

- **Single endpoint** — all operations go through one POST endpoint, unlike REST's many URL patterns
- **Request body carries the query** — no URL-encoded SOQL; instead a JSON body with `query`, `variables`, and `operationName`
- **GraphQL error model** — HTTP 200 can contain errors in the response body; three response states exist (data-only, errors-only, partial success with both)
- **Salesforce-specific wrapping** — all query data is namespaced under `data.uiapi.query.{SObject}`, and field values are wrapped objects (e.g., `Name: { value: "Acme", displayValue: null }`)

The key design challenge is error handling: the existing `Session::send_request_and_decode` helper only checks HTTP status codes, but GraphQL requires inspecting the response body for errors even on HTTP 200.

## Decision

### 1. Dedicated handler, not RestOperation

`GraphqlHandler<A>` does **not** implement `RestOperation`. The GraphQL API has no `sobjects/` URL pattern, no CRUD verbs, and no describe operations — it's a fundamentally different interaction model. This follows the same reasoning as the UI API (ADR-020).

### 2. New `ForceError::GraphQL` variant (feature-gated)

GraphQL errors are structurally different from REST API errors:

```
// REST API error:
{ "message": "...", "errorCode": "INVALID_FIELD", "fields": ["Name"] }

// GraphQL error:
{ "message": "...", "locations": [{"line": 1, "column": 45}], "path": ["uiapi", "query"], "extensions": {"errorCode": "..."} }
```

Mapping GraphQL errors to `ApiError` would lose `locations`, `path`, and `extensions` data. A dedicated `ForceError::GraphQL(GraphqlErrorResponse)` variant preserves the full error structure:

```rust
#[cfg(feature = "graphql")]
#[error("GraphQL error: {0}")]
GraphQL(#[from] crate::api::graphql::GraphqlErrorResponse),
```

### 3. Custom deserialization pipeline

The `query` method uses `Session::execute_request` (which provides retry, rate limiting, and 401 token refresh middleware) but does **not** use `Session::send_request_and_decode`. Instead, it manually:

1. Checks HTTP status (non-200 → standard HTTP error)
2. Deserializes the full `GraphqlResponse<T>` envelope
3. Inspects `data` and `errors` fields to determine success/failure

```rust
match (envelope.data, envelope.errors) {
    (Some(data), _) => Ok(data),           // Data present → success (even with warnings)
    (None, Some(errors)) => Err(errors),    // Errors only → fail
    (None, None) => Err(InvalidInput),      // Neither → unexpected
}
```

### 4. Dual query API for ergonomics

Two query methods serve different use cases:

- **`query<T>(request)`** — the 90% case. Returns `T` (the `data` field). If data is present alongside errors (partial success), returns the data. If only errors are present, returns `Err(ForceError::GraphQL(...))`.

- **`query_with_errors<T>(request)`** — returns the full `GraphqlResponse<T>` envelope, letting callers inspect both `data` and `errors`. Use this for partial-success handling.

A convenience `query_raw(query_str, variables)` method avoids constructing `GraphqlRequest` for simple cases.

### 5. `GraphqlRequest` with builder methods (not a separate builder)

Three fields don't warrant a separate builder struct. Instead, `GraphqlRequest::new(query)` with chainable `.with_variables()` and `.with_operation_name()` provides sufficient ergonomics:

```rust
let req = GraphqlRequest::new("query($limit: Int) { ... }")
    .with_variables(json!({"limit": 10}))
    .with_operation_name("GetAccounts");
```

## Module Structure

```
api/graphql/
  mod.rs      GraphqlHandler struct + query/query_with_errors/query_raw + URL resolver
  types.rs    GraphqlRequest, GraphqlResponse<T>, GraphqlError, GraphqlErrorLocation
  error.rs    GraphqlErrorResponse wrapper (Display + Error + Into<ForceError>)
```

Three files for a single-endpoint API. Splitting further would be premature.

## Consequences

**Positive:**
- Full GraphQL error information preserved (locations, path, extensions)
- Callers choose their error-handling granularity (simple vs. envelope)
- Same middleware benefits (retry, rate limiting, token refresh) as all other handlers
- No new external dependencies — uses existing `serde_json` and `reqwest`
- Feature-gated: zero cost when not used

**Negative:**
- One additional `ForceError` variant (feature-gated, so no impact when disabled)
- Cannot use `send_request_and_decode` — slightly more code in the handler. However, this is inherent to the GraphQL error model and not something we can abstract away without losing correctness.

## Alternatives Considered

**Map GraphQL errors to `ApiError`** — Rejected. Would lose `locations`, `path`, and `extensions` data. The error structures are semantically different.

**Always return `GraphqlResponse<T>` envelope** — Rejected. Makes the common case verbose. Most callers want `T`, not `(Option<T>, Option<Vec<Error>>)`.

**Use a GraphQL client crate (e.g., `graphql_client`, `cynic`)** — Rejected. Those crates are designed for schema-first codegen workflows. The Salesforce GraphQL API is better served by a thin POST wrapper that lets callers write query strings directly. Adding a schema crate would introduce heavy dependencies for minimal benefit.

**Single `query` method returning `Result<(T, Vec<GraphqlError>)>`** — Rejected. Non-standard return type, breaks the `Result<T>` convention used everywhere else in the crate, and callers would need to destructure every call even when they don't care about warnings.
