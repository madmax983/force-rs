# SOQL Query Implementation - Work in Progress

## Overview
Implementing SOQL query execution with automatic pagination using Streams for the force-rs Salesforce API client.

## Status: RED Phase Complete (TDD)

### Completed Work

#### 1. Module Structure
- Created `/c/Users/markm/force-rs/crates/force/src/api/rest/query.rs`
- Added query module to REST API handler
- Defined QueryStream<T, A> for pagination

#### 2. Type Definitions
```rust
pub struct QueryStream<T, A: crate::auth::Authenticator> {
    client: ForceClient<A>,
    next_url: Option<String>,
    current_batch: Vec<T>,
    done: bool,
    _marker: std::marker::PhantomData<T>,
}
```

#### 3. Public API Methods
```rust
impl<A: crate::auth::Authenticator> ForceClient<A> {
    // Single page query
    pub async fn query<T>(&self, soql: &str) -> Result<QueryResult<T>, ForceError>
    where T: DeserializeOwned;

    // Auto-paginating stream
    pub fn query_all<T>(&self, soql: &str) -> QueryStream<T, A>
    where T: DeserializeOwned + Unpin;
}
```

#### 4. Tests Written (6 tests)
1. `test_query_result_deserialization_typed` - Typed SObject deserialization
2. `test_query_result_deserialization_dynamic` - DynamicSObject deserialization
3. `test_query_result_with_pagination` - Pagination metadata handling
4. `test_query_result_empty` - Empty result sets
5. `test_query_result_with_nested_query` - Complex SOQL with subqueries
6. `test_query_stream_type_safety` - Compile-time type bounds validation

#### 5. Error Handling
- Added `ForceError::NotImplemented` variant
- Proper error propagation setup

### Current Blockers

Cannot proceed to GREEN phase (implementation) due to compilation errors in dependencies:

1. **Error Module Issues**
   - `error::Error` doesn't exist (should be `error::ForceError`)
   - Affects other modules using incorrect error type

2. **AccessToken API**
   - Missing `access_token()` method
   - Needed for HTTP request authentication

3. **HTTP Client Integration**
   - reqwest::Error not converting to ForceError
   - Need to complete HTTP executor integration

### Next Steps (GREEN Phase)

Once blockers resolved:

1. **Implement query() method:**
   ```rust
   - Construct query URL with SOQL parameter
   - Get access token from TokenManager
   - Execute HTTP GET request
   - Deserialize QueryResult<T>
   - Handle API errors (400/401/403)
   ```

2. **Implement QueryStream pagination:**
   ```rust
   - poll_next() should:
     * Return records from current_batch
     * Fetch next page when batch empty
     * Use next_records_url for pagination
     * Handle network errors gracefully
   ```

3. **Integration tests with wiremock:**
   - Full request/response cycle
   - Multi-page pagination
   - Error scenarios

### REFACTOR Phase (Future)

Optimizations to consider:

1. **Batch size tuning** - Configurable page size
2. **Prefetching** - Fetch next page while consuming current
3. **Connection pooling** - Reuse HTTP connections
4. **Retry logic** - Handle transient failures
5. **Query builder** - Type-safe SOQL construction

## File Locations

- **Implementation:** `crates/force/src/api/rest/query.rs` (279 lines)
- **Types:** `crates/force/src/types/query.rs` (existing QueryResult)
- **Error:** `crates/force/src/error.rs` (added NotImplemented)

## Dependencies

Blocked by these team members:
- **handler-specialist** - HTTP client integration
- **api-specialist** - REST handler completion
- **Team lead** - Access token API decisions

## Test Coverage Target

- Target: 85-90% coverage (per CLAUDE.md standards)
- Current: Types tested, implementation pending
- Planned: 15-20 integration tests with wiremock

## Notes

Following TDD strictly:
- ✅ RED: Tests written first
- ⏳ GREEN: Blocked on dependencies
- ⏳ REFACTOR: Pending implementation

All code follows Rust 2024 edition, clippy pedantic/nursery lints.
