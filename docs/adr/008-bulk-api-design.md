# ADR-008: Bulk API 2.0 Design Decisions

**Status:** Accepted
**Date:** 2026-02-08
**Deciders:** Phase 3 Team
**Context:** Bulk API 2.0 handler implementation with typestate pattern and streaming

## Context and Problem Statement

The Bulk API 2.0 provides high-volume data operations for Salesforce. It requires careful state management for job lifecycles and efficient handling of large CSV datasets.

**Problem:** How should we design the Bulk API handler to ensure compile-time safety, memory efficiency, and ease of use?

## Decision Drivers

- **Compile-time safety** - Prevent invalid job state transitions
- **Memory efficiency** - Handle 100MB+ CSV files without loading into memory
- **Ergonomics** - Simple API for common cases, advanced control when needed
- **Consistency** - Follow patterns established in Phase 2 (REST API)
- **Testability** - Comprehensive testing with wiremock
- **Binary size** - Optional feature for users who don't need bulk operations

## Bulk API 2.0 Capabilities

The Salesforce Bulk API 2.0 provides:

### 1. Ingest Jobs
- **Insert** - Bulk create new records
- **Update** - Bulk modify existing records
- **Upsert** - Bulk insert or update based on external ID
- **Delete** - Bulk remove records
- **Hard Delete** - Bulk permanently delete (bypass recycle bin)

### 2. Bulk Queries
- **Query** - Execute SOQL queries for large result sets
- **Streaming Results** - Download results as CSV stream

### 3. Job Management
- **Create Job** - Initialize bulk operation
- **Upload Data** - Send CSV data to job
- **Close Job** - Mark upload complete and start processing
- **Poll Job** - Check job status
- **Get Results** - Download successful/failed records

## Decisions

### Decision 1: Typestate Pattern for Job Lifecycle

**Decision:** Use the typestate pattern to encode job states in the type system.

**Rationale:**
- Compile-time prevention of invalid state transitions
- Can't upload to a closed job - compiler catches it
- Self-documenting through types
- IDE shows only valid methods for current state

**Trade-offs:**
- **Pro:** Compile-time safety eliminates entire class of runtime errors
- **Pro:** IDE autocomplete guides users to correct next steps
- **Con:** Slightly more verbose (requires variable rebinding)
- **Con:** Generic parameters visible in function signatures

**Alternatives Considered:**
- Runtime state validation - Rejected: errors only caught at runtime, no IDE guidance
- Single Job type with state enum - Rejected: allows calling any method at any time

**Implementation:**
```rust
// State transitions through types
let job = handler.create_ingest_job("Account", JobOperation::Insert)
    .await?;                                   // Returns IngestJob<Open, A>

let job = job.upload(&csv_data).await?;        // Open → UploadComplete
let job = job.close().await?;                  // UploadComplete → InProgress
let job = job.poll_until_complete().await?;    // InProgress → JobComplete
```

### Decision 2: Arc<Inner<A>> Resource Sharing

**Decision:** Share HTTP client and token manager across handlers using `Arc<Inner<A>>`.

**Rationale:**
- Zero-cost resource sharing (no cloning)
- Avoids requiring `A: Clone` on authenticators
- Maintains efficient connection pooling
- Consistent with RestHandler pattern from Phase 2

**Trade-offs:**
- **Pro:** Efficient resource usage, no duplication
- **Pro:** Works with any authenticator (no Clone bound)
- **Con:** Requires Arc indirection (negligible overhead)

**Alternatives Considered:**
- Clone entire HTTP client per handler - Rejected: wastes resources, breaks connection pooling
- Require `A: Clone` - Rejected: unnecessary constraint on authenticators

**Implementation:**
```rust
pub struct BulkHandler<A: Authenticator> {
    pub(crate) inner: Arc<crate::client::Inner<A>>,
}
```

### Decision 3: Feature Gates for Bulk API

**Decision:** Place all Bulk API code behind `#[cfg(feature = "bulk")]`.

**Rationale:**
- Users only compile what they need
- Reduces binary size for REST-only users
- Clear separation of concerns
- Follows Cargo best practices

**Trade-offs:**
- **Pro:** Smaller binaries for users who don't need bulk operations
- **Pro:** Faster compilation for REST-only use cases
- **Con:** Requires feature flag in Cargo.toml to use

**Alternatives Considered:**
- Always-on bulk API - Rejected: increases binary size unnecessarily for all users

**Configuration:**
```toml
[features]
default = ["rest"]
rest = []
bulk = ["csv"]
```

### Decision 4: Convenience Methods for Common Operations

**Decision:** Provide high-level convenience methods alongside low-level API access.

**Rationale:**
- 90% of users want simple insert/update/query
- 10% need fine-grained control
- Progressive disclosure: simple → builder → direct API

**Methods Provided:**
```rust
// Simple convenience methods (handles lifecycle automatically)
bulk_insert(object, records)    // Insert + wait
bulk_update(object, records)    // Update + wait
bulk_delete(object, ids)        // Delete + wait
bulk_query(soql)                // Query + stream results

// Advanced: Manual lifecycle control
create_ingest_job(object, op)   // Typed job creation
create_upsert_job(object, id)   // Typed upsert creation
create_job(request)             // Fine-grained control
upload(csv)
close()
poll_until_complete()
```

**Trade-offs:**
- **Pro:** Simple API for common cases
- **Pro:** Advanced users can use lower-level methods
- **Con:** More API surface to document

## Consequences

### Positive

- **Compile-time safety** - Invalid state transitions caught by compiler
- **Memory efficient** - Streaming CSV prevents loading 100MB+ files into RAM
- **Ergonomic** - Simple methods for 90% of use cases
- **Consistent** - Follows Phase 2 patterns (Arc<Inner<A>>, handler pattern)
- **Testable** - Comprehensive test coverage with wiremock
- **Optional** - Feature-gated to reduce binary size

### Negative

- **Learning curve** - Typestate pattern unfamiliar to some developers
- **Verbosity** - Variable rebinding required for state transitions
- **Compilation time** - Feature gates add conditional compilation complexity

## Testing

Phase 3 added 72 new tests:
- Task #1 (Foundation): 22 tests
- Task #2 (Ingest): 18 tests
- Task #3 (Query): 17 tests
- Task #4 (CSV): 15 tests

All tests use wiremock for HTTP mocking, ensuring reliable, fast tests.

## Related ADRs

- [ADR-006: REST API Handler Pattern](006-handler-pattern.md) - Established Arc<Inner<A>> pattern
- [ADR-007: REST API Design](007-rest-api-design.md) - REST API implementation patterns

## References

- [Salesforce Bulk API 2.0 Documentation](https://developer.salesforce.com/docs/atlas.en-us.api_bulk_v2.meta/api_bulk_v2/)
- [Typestate Pattern in Rust](http://cliffle.com/blog/rust-typestate/)
