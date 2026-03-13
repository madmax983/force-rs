# ADR-018: Composite API Design

**Status:** Proposed
**Date:** 2026-03-10
**Deciders:** Codex, Atlas
**Context:**
The core library needed to support the Salesforce Composite API, specifically the Composite Batch and Composite Graph APIs. These APIs allow executing multiple REST requests in a single HTTP call to reduce network overhead and API limits usage. We needed a structured, performant way to build and submit these accumulated payloads.

**Decision Drivers:**
- **Performance:** Avoid unnecessary heap allocations when collecting many sub-requests.
- **Limits Enforcement:** Respect Salesforce limits for Batch and Graph endpoints.
- **Ergonomics:** Provide a builder pattern to easily compose complex request trees.

**Decision:**
We implemented the Composite API handlers in `crates/force/src/api/composite/` utilizing builder patterns (`BatchBuilder`, `GraphBuilder`).
To optimize performance, we explicitly pre-allocate capacity for the underlying data structures:
- `BatchBuilder` pre-allocates a `Vec` with capacity for 25 requests, which is the hard limit for the Salesforce Composite Batch API.
- `GraphBuilder` and `Graph` pre-allocate a `Vec` with capacity for 15 elements, optimizing for the typical small graph size and avoiding heap reallocations during accumulation.

**Consequences:**

### Positive
- **Performance:** Pre-allocating `Vec` capacity prevents costly heap reallocations as requests are appended to builders.
- **Safety:** Structuring the API via builders enforces a unified construction path.

### Negative
- **Memory Overhead:** A tiny bit of memory is eagerly allocated even if the user only intends to send 1 or 2 requests in a batch/graph, though this is negligible in practice.

## Related Decisions
- [ADR-004: Feature Gates](004-feature-gates.md) (Composite API is feature-gated).
- [ADR-006: Handler Pattern](006-handler-pattern.md) (Exposing API via `client.composite()`).
