# ADR-009: HTTP Resiliency & Telemetry

**Status:** Accepted
**Date:** 2026-02-07
**Deciders:** Codex, Force-RS Team
**Context:** Distributed system reliability and observability

## Context and Problem Statement

The `force-rs` library operates in a distributed environment where network failures, API rate limits, and authentication expirations are expected. Without a robust strategy for handling these transient failures, applications using the library would be fragile, requiring manual retry logic and error handling code at every call site.

Furthermore, debugging issues in production (e.g., "why is this request slow?", "are we being rate limited?") requires visibility into the internal behavior of the HTTP client, including retry attempts and backoff durations.

We need a unified approach to:
1.  Handle transient network errors (timeouts, connection resets).
2.  Respect Salesforce API rate limits (429 Too Many Requests).
3.  Automatically refresh expired authentication tokens (401 Unauthorized).
4.  Provide telemetry for monitoring and debugging.

## Decision Drivers

*   **Reliability:** The client should recover from transient failures automatically.
*   **Correctness:** Rate limits must be respected to avoid IP bans or long suspension.
*   **Observability:** Users need to know when retries happen and why.
*   **Simplicity:** The public API should hide the complexity of retries.
*   **Performance:** Backoff strategies should minimize load on the server while recovering quickly.

## Decision Outcome

**Chosen:** Implement a middleware-based `HttpExecutor` that encapsulates retry, auth refresh, and telemetry logic.

### 1. Middleware Architecture

We introduce `HttpExecutor` as the central component for executing all HTTP requests. It wraps the `reqwest::Client` and applies the following logic layer:

```mermaid
sequenceDiagram
    participant Client
    participant HttpExecutor
    participant Salesforce

    Client->>HttpExecutor: execute(request)
    loop Retry Loop
        HttpExecutor->>HttpExecutor: Inject Auth Token
        HttpExecutor->>Salesforce: HTTP Request
        alt Success (2xx)
            Salesforce-->>HttpExecutor: Response
            break
        else 401 Unauthorized
            HttpExecutor->>HttpExecutor: Refresh Token
            note right of HttpExecutor: Retry immediately
        else 429 Too Many Requests
            Salesforce-->>HttpExecutor: Retry-After Header
            HttpExecutor->>HttpExecutor: Sleep(Retry-After)
            note right of HttpExecutor: Retry after delay
        else 503 Service Unavailable / Network Error
            HttpExecutor->>HttpExecutor: Exponential Backoff
            note right of HttpExecutor: Retry with jitter
        end
    end
    HttpExecutor-->>Client: Result<Response>
```

### 2. Retry Policy

We distinguish between different types of requests using `RequestRetryClass`:
*   **Read (GET/HEAD):** Safe to retry multiple times (default: 3).
*   **Idempotent Mutation:** Safe to retry (default: 3).
*   **Mutation (POST/PATCH/DELETE):** Not safe to retry automatically (default: 0) unless explicitly marked idempotent.

### 3. Telemetry Hooks

Instead of tightly coupling with a logging framework, `HttpExecutor` exposes `TelemetryHooks`. This allows consumers to inject custom callbacks for:
*   `on_retry(&RetryEvent)`: Triggered when a request is retried.
*   `on_complete(&RequestCompletion)`: Triggered when a request finishes (success or failure).

This design allows `force-rs` to be observability-agnostic, supporting `tracing`, `log`, or custom metrics collectors.

## Consequences

### Positive
*   ✅ **Robustness:** Applications are resilient to transient failures by default.
*   ✅ **Compliance:** Automatically respects `Retry-After` headers, preventing API abuse.
*   ✅ **Developer Experience:** Users don't need to write `loop { try_request() }` boilerplate.
*   ✅ **Observability:** Detailed insights into retry behavior and latency.

### Negative
*   ⚠️ **Latency:** Failed requests take longer to return an error due to retries.
*   ⚠️ **Complexity:** The `HttpExecutor` is a complex state machine.

### Neutral
*   ℹ️ **Configuration:** Users must configure `max_retries` and `timeout` appropriately for their workload.
