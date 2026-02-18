# ADR-011: Streaming Patterns and Cleanup

**Status:** Accepted
**Date:** 2026-02-09
**Deciders:** Codex, Atlas, Jules
**Context:** Efficient Data Handling and Zombie Code Cleanup

## Context and Problem Statement

The Force SDK must handle large datasets efficiently without blocking threads or consuming excessive memory.
Specifically:
1.  **Bulk API 2.0 Ingest:** Users need to upload millions of records. Loading all records into memory to create a CSV payload is inefficient and prone to OOM errors.
2.  **SOQL Queries:** Query results can be large and paginated. Fetching all pages into a single `Vec` before processing is inefficient.
3.  **Zombie Code:** The `pub_sub` module was a placeholder with heavy dependencies (`tonic`, `prost`) that were not being used, violating the "Zombie Code" philosophy.

**Problem:** How do we provide ergonomic, memory-safe abstractions for large data operations while keeping the codebase lean?

## Decision Drivers

-   **Memory Efficiency:** Streaming data processing (O(1) memory vs O(N)).
-   **Ergonomics:** Easy-to-use APIs for common tasks.
-   **Maintainability:** Remove unused code to reduce technical debt.
-   **Architectural Transparency:** Explicitly document streaming patterns.

## Decisions

### Decision 1: SmartIngest for Bulk Uploads

**Decision:** Implement `SmartIngest` as a high-level utility for Bulk API 2.0.

**Pattern:** Stream-to-Batch Adapter.
-   Accepts an async stream of serializable records.
-   Buffers records internally until a batch size (default 10k) is reached.
-   Serializes the buffer to CSV.
-   Uploads the batch to Salesforce.
-   Repeats until the stream is exhausted.

**Rationale:**
-   Decouples data generation from upload logic.
-   Prevents holding the entire dataset in memory.
-   Automates the job lifecycle (Create -> Upload Batches -> Close -> Poll).

### Decision 2: QueryStream for SOQL Results

**Decision:** Implement `QueryStream` as an async iterator for SOQL queries.

**Pattern:** Async Iterator / Paginator.
-   Lazily fetches pages of results from Salesforce.
-   Iterates over records within the current page.
-   Automatically fetches the next page when the current one is exhausted.
-   Uses `std::vec::IntoIter` to avoid moving elements into a temporary buffer.

**Rationale:**
-   Allows processing records as they arrive.
-   Abstracts away pagination details (`nextRecordsUrl`).
-   Standard Rust pattern (`futures::Stream`).

### Decision 3: Remove Pub/Sub Zombie Code

**Decision:** Remove the `pub_sub` feature, module, and dependencies (`tonic`, `prost`, `apache-avro`).

**Rationale:**
-   The module was a placeholder with no implementation.
-   It pulled in heavy dependencies even if unused (if the feature was enabled).
-   Future implementation should be built from scratch based on actual requirements.
-   Adheres to the "Zombie Code" removal policy.

## Consequences

### Positive

-   **Reduced Bloat:** Removed ~3 heavy crate dependencies from the dependency tree (when `all` was selected).
-   **Memory Safety:** Users can process arbitrarily large datasets with constant memory usage.
-   **Clarity:** Dedicated patterns for streaming data in/out of Salesforce.
-   **Transparency:** No misleading "placeholder" features.

### Negative

-   **Breaking Change:** Users relying on the `pub_sub` feature flag (even if empty) will need to update their `Cargo.toml`.
-   **Complexity:** Streaming implementations (pinning, futures) are more complex than synchronous buffering.

## Compliance

-   **Grandma Test:** "We treat data like a river, not a bucket, and we threw away the broken toys nobody was playing with."
