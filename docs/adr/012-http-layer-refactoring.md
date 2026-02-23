# ADR-012: HTTP Layer Refactoring & Observability

**Status:** Accepted
**Date:** 2026-02-17
**Deciders:** Codex, Atlas
**Context:**
The core HTTP handling logic within the `force` crate was growing in complexity. It was responsible for:
1.  **Authentication Injection:** Adding Bearer tokens to requests.
2.  **Retry Logic:** Handling transient failures (503, 429) and exponential backoff.
3.  **Observability:** Logging requests, retries, and errors.
4.  **Error Handling:** Parsing Salesforce-specific error responses.

Previously, these concerns were somewhat entangled or monolithic. To improve maintainability and testability, we needed to decompose the HTTP layer into focused sub-modules. Additionally, there was a need to modernize observability using the `tracing` ecosystem while retaining the programmatic hooks for metrics collection.

**Decision:**
We have decomposed the `crates/force/src/http` module into four distinct components:
1.  **`executor`**: The orchestrator (`HttpExecutor`) that manages the request lifecycle, middleware application, and retry loop.
2.  **`retry`**: Dedicated logic (`RetryPolicy`, `RequestRetryClass`) for determining when and how to retry requests.
3.  **`telemetry`**: Structures (`TelemetryHooks`, `TelemetryContext`) for capturing request metrics and events.
4.  **`error`**: logic for mapping HTTP responses to `ForceError`.

**Observability Strategy:**
We have adopted a **Hybrid Observability** model:
-   **Tracing (Internal/Diagnostic):** The `executor` uses the `tracing` crate to emit structured logs (`info!`, `warn!`, `error!`). This provides standard, zero-config logging for debugging and monitoring.
-   **Telemetry Hooks (External/Programmatic):** The `TelemetryHooks` API allows consumers to register callbacks (`on_retry`, `on_complete`). This enables programmatic access to request metrics without parsing logs, useful for integrating with custom monitoring systems (e.g., Prometheus counters, Datadog).

**Consequences:**
### Positive
-   **Separation of Concerns:** Each module has a single responsibility, making the code easier to understand and test.
-   **Flexible Observability:** Users can choose between standard logging (tracing) or custom metric collection (hooks), or both.
-   **Maintainability:** Changes to retry policies or error parsing are isolated from the main execution loop.

### Negative
-   **Module Complexity:** The `http` module now has more files and internal structure than a single `client.rs`.
-   **API Surface:** `TelemetryHooks` adds to the public API surface area that must be maintained.
