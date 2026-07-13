# Operations

Runtime behavior of the HTTP layer: timeouts, retries, rate limiting, pooling,
in-path token refresh, the error taxonomy, API-version pinning, and
observability. All knobs live on `ClientConfig` (`crates/force/src/config.rs`)
and the `HttpExecutor` (`crates/force/src/http/executor.rs`).

`ClientConfig` is a plain struct with public fields, constructed by literal +
`..Default::default()`. There is no fluent `ClientConfig::builder()`.

```rust
use std::time::Duration;
use force::config::{ClientConfig, Environment};

let config = ClientConfig {
    api_version: "v67.0".to_string(),
    environment: Environment::Production,
    timeout: Duration::from_secs(30),
    max_retries: 3,
};

let client = ForceClientBuilder::new()
    .config(config)
    .authenticate(auth)
    .build()
    .await?;
```

Defaults (`ClientConfig::default`): `api_version = "v67.0"`, `environment =
Production`, `timeout = 30s`, `max_retries = 3`.

## Timeouts

`ClientConfig` exposes a single `timeout: Duration`. At build time it is applied
in two places (`crates/force/src/client/builder.rs`):

- `reqwest::Client::builder().timeout(config.timeout)` — the reqwest end-to-end
  request timeout.
- `HttpExecutor::with_client(client, max_retries, config.timeout)` — the
  executor wraps each attempt in `tokio::time::timeout(self.timeout, ...)`.

So `timeout` is a **per-attempt request timeout**, not a whole-call budget: each
retry gets a fresh `timeout` window. A timeout produces
`HttpError::Timeout { timeout_seconds }` and is treated as a retryable transient
failure (see below).

There is **no separate connect timeout** in `ClientConfig`; connection
establishment falls under reqwest's defaults. The default executor base backoff
is `500ms` (`BASE_BACKOFF_MS`), configurable only via
`HttpExecutor::with_base_backoff`.

## Retries & backoff

Retry counts are governed by `RetryPolicy` (`crates/force/src/http/retry.rs`),
which distinguishes three request classes:

| Class | `RetryPolicy` field | Source |
| --- | --- | --- |
| `Read` (GET/HEAD/OPTIONS/TRACE) | `read_max_retries` | `classify_request` |
| `IdempotentMutation` | `idempotent_mutation_max_retries` | explicit opt-in |
| `Mutation` (POST/PATCH/PUT/DELETE/CONNECT) | `mutation_max_retries` | `classify_request` |

The public builder wires the executor via
`HttpExecutor::with_client(client, max_retries, timeout)`, which calls
`RetryPolicy::new(max_retries, 0)`. That means, out of the box:

- **Reads** retry up to `config.max_retries` (default 3).
- **Mutations** retry **0** times — non-idempotent writes are never
  auto-retried, to avoid duplicate side effects.
- `idempotent_mutation_max_retries` defaults to `read_max_retries`; a POST can
  be marked idempotent via
  `RetryPolicy::with_idempotent_mutation_retries` + the executor's
  `execute_response_with_retry_class` path.

**What is retried** (`retry_loop` / `is_retryable_error`):

- `HttpError::Timeout` (per-attempt timeout).
- Transport `HttpError::RequestFailed(reqwest::Error)` that is not a builder,
  redirect, or status error (i.e. connect/read failures).
- HTTP `503 Service Unavailable`.

**Not retried by the retry counter:** `401` (single refresh+retry, separate
path) and `429` (surfaced as an error — see Rate limiting).

**Backoff** is exponential with a 30s ceiling
(`exponential_backoff(attempt, base)` = `base * 2^attempt`, capped at
`MAX_BACKOFF_MS = 30_000`, or `base` if `base` is larger). With the default
`500ms` base the delay sequence per attempt is:

```
500, 1000, 2000, 4000, 8000, 16000, 30000  (capped)
```

Each retry sleeps `tokio::time::sleep(backoff)` and emits a `tracing::warn!`
carrying `retry.attempt`, `http.status_code`, `retry.backoff_ms`.

Tune `max_retries` per client via `ClientConfig`. See the
[retry-tuning runbook](../runbooks/retry-tuning.md).

## Rate limiting

A `429 Too Many Requests` is handled by `handle_rate_limit`. It does **not**
loop on the retry counter. Instead it:

1. Parses the `Retry-After` response header via `parse_retry_after` (integer
   seconds; missing/invalid → `60`).
2. Returns `HttpError::RateLimitExceeded { retry_after_seconds }`.

The caller is responsible for backing off and re-issuing after
`retry_after_seconds`. Salesforce enforces a per-org rolling **24-hour** request
allocation (`REQUEST_LIMIT_EXCEEDED`); a `429` means the org-wide budget, not a
per-request throttle, is exhausted, so blind local retries will not help. During
an incident, drain non-critical callers and honor `Retry-After`. See the
[rate-limit incident runbook](../runbooks/rate-limit-incident.md).

## Connection pooling

The client uses a single `reqwest::Client` built in
`AuthenticatedBuilder::build`. The `reqwest::Client` owns an internal
connection pool with keep-alive; it is cloned (cheap, `Arc`-backed) and shared
across the `Session` and all API handlers, so TCP/TLS connections are reused
across every `client.rest()`, `client.bulk()`, etc. call.

The builder does not currently set an explicit pool size or idle limit —
reqwest's defaults apply (idle connections kept per host, reused on subsequent
requests). Pool sizing is therefore not exposed through `ClientConfig`; changing
it requires constructing the executor directly with
`HttpExecutor::with_client(custom_reqwest_client, ...)`.

## Token refresh in the request path

Token refresh happens inline inside `retry_loop`, distinct from the transient
retry counter:

1. Every attempt injects `Authorization: Bearer <token>`
   (`inject_auth_header`).
2. On `401 Unauthorized`, if a refresh has **not** already been attempted for
   this call, the executor invokes the `refresh_token` callback (backed by the
   `TokenManager`), re-injects the new bearer header, and retries **once**.
3. If the retried request also returns `401`, `execute` maps it to
   `HttpError::StatusError { status_code: 401, message: "Unauthorized after
   token refresh" }` rather than looping.

This makes expiry recovery transparent to callers: a stale access token is
refreshed and the request replayed without surfacing an error. The refresh
mechanics and which flows support refresh tokens are covered in
[Choosing an auth flow](02-choosing-an-auth-flow.md).

## Error taxonomy & handling

All fallible operations return `force::error::Result<T>` (alias for
`Result<T, ForceError>`). Errors use `thiserror`; production code contains no
`unwrap()`/`expect()`. `ForceError` (`crates/force/src/error.rs`):

| Variant | Payload | Meaning |
| --- | --- | --- |
| `Authentication` | `AuthenticationError` | Token/credential failures. |
| `Http` | `HttpError` | Transport, status, timeout, rate-limit. |
| `Api` | `ApiError` | Salesforce error body (`errorCode`/`message`/`fields`). |
| `Config` | `ConfigError` | Missing/invalid configuration, env vars. |
| `Serialization` | `SerializationError` | JSON/CSV/format failures. |
| `InvalidId` | `SalesforceIdError` | 15/18-char ID validation. |
| `InvalidInput` | `String` | Bad argument to an API method. |
| `Io` | `std::io::Error` | I/O. |
| `NotImplemented` | `String` | Unimplemented surface. |
| `GraphQL` *(feature `graphql`)* | `GraphqlErrorResponse` | GraphQL envelope errors. |
| `Cpq` *(feature `cpq`)* | `CpqErrorResponse` | CPQ service errors. |

Sub-taxonomies:

- **`AuthenticationError`**: `TokenRequestFailed`, `InvalidCredentials`,
  `TokenExpired`, `TokenRefreshFailed`, `InvalidToken`, and (feature `jwt`)
  `JwtCreationFailed`, `InvalidJwtConfig`.
- **`HttpError`**: `RequestFailed(reqwest::Error)`, `StatusError { status_code,
  message }`, `RateLimitExceeded { retry_after_seconds }`, `Timeout {
  timeout_seconds }`, `InvalidUrl`, `RequestBuildError`, `PayloadTooLarge {
  limit_bytes }`.
- **`ApiError`**: struct `{ message, error_code, fields }`. Salesforce error
  arrays are parsed by `parse_api_error`; the first element's `errorCode`,
  `message`, and `fields` are formatted into the error.
- **`ConfigError`**: `MissingValue`, `InvalidValue { field, reason }`, `EnvVar`.
- **`SerializationError`**: `Json`, `InvalidFormat`, and (feature `bulk`) `Csv`.

Handling pattern — match the top-level variant, then drill into `HttpError` for
operational decisions:

```rust
use force::error::{ForceError, HttpError, AuthenticationError};

match client.rest().query(soql).await {
    Ok(result) => { /* ... */ }

    // Org-wide 24h limit hit: back off for the advertised window.
    Err(ForceError::Http(HttpError::RateLimitExceeded { retry_after_seconds })) => {
        tokio::time::sleep(std::time::Duration::from_secs(retry_after_seconds)).await;
    }

    // Per-attempt timeout after retries were exhausted.
    Err(ForceError::Http(HttpError::Timeout { timeout_seconds })) => {
        tracing::warn!(timeout_seconds, "request timed out");
    }

    // Non-2xx Salesforce response (401 after refresh, 4xx/5xx).
    Err(ForceError::Http(HttpError::StatusError { status_code, message })) => {
        tracing::error!(status_code, %message, "salesforce returned an error status");
    }

    // Refresh exhausted / credentials rejected.
    Err(ForceError::Authentication(AuthenticationError::TokenRefreshFailed(msg))) => {
        tracing::error!(%msg, "token refresh failed; rotate or re-auth");
    }

    // Parsed Salesforce API error body.
    Err(ForceError::Api(api)) => {
        tracing::error!(code = %api.error_code, msg = %api.message, "api error");
    }

    Err(other) => return Err(other.into()),
}
```

Each `HttpError` corresponds to an HTTP disposition: `429 →
RateLimitExceeded`, per-attempt timeout `→ Timeout`, any other non-2xx `→
StatusError` (with the Salesforce `errorCode`/`message` folded into `message`).

## API version pinning

`ClientConfig.api_version` is a `String` in `vXX.0` format. The default is
derived from the `ApiVersion` newtype (`crates/force/src/types/api_version.rs`):
`ApiVersion::DEFAULT.to_string()` = **`v67.0`**. (Some tests exercise other
versions such as `v62.0`; the shipped default is `v67.0`.)

Pin per client by setting the field:

```rust
let config = ClientConfig { api_version: "v66.0".to_string(), ..Default::default() };
```

`ApiVersion` provides validation and a support contract:

- Parse/format: `"v60.0".parse::<ApiVersion>()`, `Display` → `v60.0`,
  `major()` → `60`. Parsing rejects a missing `v` prefix, a missing `.0`
  suffix, or a zero/non-numeric major.
- Constants: `V55`..`V67`, `MIN_SUPPORTED = V55`, `MAX_TESTED = V67`,
  `DEFAULT = V67`.
- Tiers via `support_tier()` → `ApiVersionSupportTier`:
  - `Tested` — within `[V55, V67]` (`is_tested()`).
  - `SupportedUntested` — `>= V55` but above `MAX_TESTED` (`is_supported()`
    true, `is_tested()` false).
  - `Unsupported` — below `V55`.

Validate an intended version against the support tier before pinning, and roll
forward deliberately. See the
[API version upgrade runbook](../runbooks/salesforce-api-version-upgrade.md).
Credential rotation that may accompany a version bump is covered by the
[auth credential rotation runbook](../runbooks/auth-credential-rotation.md).

## Observability

The HTTP layer follows a hybrid model (see
[ADR-012](../adr/012-http-layer-refactoring.md)): structured `tracing` for
diagnostics plus optional programmatic hooks.

### Tracing

`execute_response_with_retry_class` opens a `force_http_request` info span with:

- `http.method`
- `http.path` (path only; query string excluded)
- `request.class` (`read` / `idempotent_mutation` / `mutation`)

Within that span the executor emits:

- On each retry — `tracing::warn!("retrying request after transient failure")`
  with `retry.attempt`, `http.status_code`, `retry.backoff_ms`.
- On completion — `tracing::info!("request completed")` with
  `http.status_code`, `retries`, `elapsed_ms`, `error.kind`.

Subscribe with `tracing-subscriber`:

```rust
tracing_subscriber::fmt()
    .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
    .init();
```

Set e.g. `RUST_LOG=force=info` (or `debug`) to capture request spans and
completion events.

### Telemetry hooks

For metric pipelines that should not parse logs, `TelemetryHooks`
(`crates/force/src/http/telemetry.rs`) exposes redaction-safe callbacks
registered on the executor via `HttpExecutor::with_telemetry_hooks`:

- `on_retry(&RetryEvent)` — `method`, `path`, `request_class`, `attempt`,
  `status_code`, `backoff_ms`.
- `on_complete(&RequestCompletion)` — `method`, `path`, `request_class`,
  `status_code: Option<u16>`, `error_kind: Option<RequestErrorKind>`,
  `retries`, `elapsed_ms`.

`RequestErrorKind` is one of `Timeout`, `Transport`, `RateLimited`. Both events
carry only the URL **path** (never the query string), so tokens and record IDs
in query parameters are not leaked into metrics.
