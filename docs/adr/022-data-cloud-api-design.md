# ADR-022: Data Cloud API Design

**Date:** 2026-03-21
**Status:** Accepted
**Feature flag:** `data_cloud`

## Context

Salesforce Data Cloud (formerly CDP) exposes a REST API at `/services/data/vXX.0/ssot/` on a **separate tenant endpoint** (e.g., `https://tenant.c360a.salesforce.com`). Unlike all other Salesforce APIs in this crate, Data Cloud requires a **two-step token exchange**:

1. Obtain a standard Salesforce platform OAuth token (via any existing `Authenticator`)
2. Exchange it for a Data Cloud-specific token via `POST {instance_url}/services/a360/token`

The DC token targets a completely different host and has its own expiration lifecycle. This creates architectural tension: the existing `Session::resolve_url()` reads `instance_url` from the current `AccessToken`, and all handlers share one `Session<A>`. A DC handler needs a different instance URL and a different token.

## Decision

### 1. Decorator Authenticator Pattern

`DataCloudAuthenticator<A: Authenticator>` wraps an existing platform authenticator by sharing its `Arc<TokenManager<A>>`. When `authenticate()` is called:

1. It calls `platform_token_manager.token().await?` to get a fresh platform token
2. POSTs the exchange request to `/services/a360/token`
3. Returns an `AccessToken` whose `instance_url` is the DC tenant URL

This design means `DataCloudAuthenticator` itself implements `Authenticator`, so it can be managed by its own `TokenManager<DataCloudAuthenticator<A>>` with independent soft/hard expiry.

**Why decorator, not separate client:** A standalone `DataCloudClient` would force users to manage two clients and manually coordinate platform tokens for the exchange. The decorator pattern keeps `ForceClient` as the single entry point while cleanly separating the DC token lifecycle.

### 2. Separate Session, Reused Infrastructure

`ForceClient<A>` gains an optional `dc_session: Option<Arc<Session<DataCloudAuthenticator<A>>>>`. The DC session reuses:

- **`Session::resolve_url()`** — works unchanged because the DC token's `instance_url` is the tenant URL
- **`Session::execute_request()`** — handles auth headers, retries, and 401 refresh
- **`Session::send_request_and_decode()`** — standard JSON response handling
- **`reqwest::Client`** — shared connection pool (cloned from platform session)

Zero modifications to `Session`. The DC handler simply wraps a `Session` that happens to be authenticated with a `DataCloudAuthenticator`.

### 3. Eager Builder Configuration

The `AuthenticatedBuilder` gains `.with_data_cloud(DataCloudConfig)`, and the DC session is created in `build()`:

```rust
let client = builder()
    .authenticate(auth)
    .with_data_cloud(DataCloudConfig::default())
    .build()
    .await?;
```

**Why eager, not lazy:** DC is an explicit opt-in with configuration. Creating the session at build time is simpler than `OnceLock` for async initialization. The session creation is cheap (no network calls until first request).

### 4. `data_cloud()` Returns `Result`

Unlike `.rest()` or `.ui()` which are infallible (the handler always exists when the feature flag is on), `.data_cloud()` returns `Result<DataCloudHandler<A>>` because DC requires runtime configuration. Calling it without `.with_data_cloud()` returns `ConfigError::MissingValue`.

### 5. URL Prefix: `ssot/`

DC API paths are resolved as `{dc_instance}/services/data/{version}/ssot/{path}`:

```rust
fn resolve_dc_url(&self, path: &str) -> Result<String> {
    self.inner.resolve_url(&format!("ssot/{path}")).await
}
```

This follows the same pattern as `UiHandler::resolve_ui_url("ui-api/{path}")`.

### 6. Error Handling: Reuse `AuthenticationError`

DC token exchange failures use `AuthenticationError::TokenRequestFailed` with a "Data Cloud token exchange failed" prefix. This avoids adding a feature-gated error variant since the exchange is fundamentally an authentication step.

## Consequences

### Positive

- **Zero changes to Session** — the decorator pattern leverages existing URL resolution and HTTP middleware
- **Shared platform token** — DC exchange reuses cached platform tokens via the shared `TokenManager`
- **Independent DC token lifecycle** — DC tokens are managed by their own `TokenManager` with independent expiry tracking
- **Familiar handler pattern** — `DataCloudHandler` follows the same `Arc<Session<A>>` pattern as all other handlers
- **API version independence** — `DataCloudConfig::api_version` can override the platform version for DC calls

### Negative

- **`ForceClient` struct grows** — gains a conditional `dc_session` field (mitigated by `#[cfg(feature = "data_cloud")]`)
- **Nested generics** — `Session<DataCloudAuthenticator<A>>` is a mouthful (but internal to the crate)
- **`data_cloud()` is fallible** — unlike other handler accessors, returns `Result` (necessary because DC requires config)

## Token Flow

```
ForceClient<A>
  |
  +-- Arc<Session<A>> (platform)
  |     +-- Arc<TokenManager<A>>
  |           |
  |           v
  |     A::authenticate() --> platform AccessToken
  |
  +-- Arc<Session<DataCloudAuthenticator<A>>> (DC)
        +-- Arc<TokenManager<DataCloudAuthenticator<A>>>
              |
              v
        DataCloudAuthenticator::authenticate()
          1. platform_token_manager.token() --> cached platform token
          2. POST /services/a360/token (exchange)
          3. --> DC AccessToken { instance_url: "https://tenant.c360a.salesforce.com" }
```
