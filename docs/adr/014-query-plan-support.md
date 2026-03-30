# ADR-014: Query Plan Support

**Status:** Accepted
**Date:** 2026-02-18
**Deciders:** Codex, Atlas
**Context:**
Salesforce enforces strict governor limits on SOQL queries. Queries that are non-selective (e.g., table scans on large objects) can fail or time out in production.
Developers often need to verify query performance characteristics (cardinality, cost, index usage) *before* deploying code. While the Salesforce Developer Console provides a "Query Plan" tool, there was no programmatic way to access this data within the SDK itself, making it difficult to automate performance checks in CI/CD pipelines.

**Decision:**
Query-plan support is part of the standard REST surface.
This provides the `client.rest().explain(soql)` method, which calls the Salesforce Query Plan API (`/services/data/vXX.X/query/?explain=...`).

The response includes:
-   **Cardinality:** Expected number of records.
-   **Relative Cost:** A normalized cost metric (lower is better).
-   **Leading Operation Type:** e.g., "TableScan" (bad) vs "IndexScan" (good).
-   **Notes:** Warnings about unindexed fields.

**Consequences:**
### Positive
-   **Performance Tooling:** Enable automated performance assertions (e.g., `assert!(plan.relative_cost < 1.0)`).
-   **No Surprise Feature Flag:** Users already on `rest` can inspect query plans without hunting for a secondary feature.

### Negative
-   **Broader REST Surface:** Query-plan helpers now ship anywhere the REST handler is available.
