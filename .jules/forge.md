**[Extract send_request_and_decode helper]**
**Learning:** Consolidating HTTP request execution, error handling, and JSON decoding into a single helper (`send_request_and_decode`) significantly reduces boilerplate and enforces consistent error handling across different API handlers.
**Action:** Look for similar patterns in other modules (e.g., Bulk API) where request execution might be duplicated.

**[Decompose Complex Retry Loops]**
**Learning:** Extracting individual steps of a complex retry loop (execution attempt, error handling, backoff) into dedicated helper methods significantly improves readability and removes the need for clippy suppressions.
**Action:** Identify other complex loops or match statements (e.g. in Bulk API streaming) and decompose them similarly.
**[Extract HTTP client wrappers]**\n**Learning:** Re-implementing HTTP request and URL formatting logic manually in  reduces maintainability and ignores centralized helpers like  and .\n**Action:** Always prefer using the provided HTTP methods directly on the  struct rather than manually accessing  and writing custom format blocks for URLs.

**[Extract HTTP client wrappers]**
**Learning:** Re-implementing HTTP request and URL formatting logic manually in `crates/force/src/api/bulk/query.rs` reduces maintainability and ignores centralized helpers like `Session::resolve_url` and `Session::get/post/patch/delete`.
**Action:** Always prefer using the provided HTTP methods directly on the `Session` struct rather than manually accessing `.http_client` and writing custom format blocks for URLs.

**[Guard Clauses over if-let nesting]**
**Learning:** `if let` blocks can easily lead to deep nesting ("Pyramid of Doom") especially when performing multi-step validations or conditional extraction.
**Action:** Prefer `let Some(..) = ... else { return ... };` guard clauses which flatten the code, making the happy-path execution flow cleaner. Applied this successfully in `QueryStream::fetch_next_page` to flatten conditional URL fetching.
**[Remove Error Handling Boilerplate]**
**Learning:** Re-implementing a thin wrapper like `handle_error_response` around a central HTTP utility (`crate::http::response_to_force_error`) adds unnecessary boilerplate and indirection across different modules (e.g., REST CRUD, Bulk Ingest, Bulk Query).
**Action:** Always prefer calling central HTTP utility functions directly (e.g., `crate::http::response_to_force_error(response, context).await`) instead of creating localized wrappers unless domain-specific mapping is required.

**[Iterator Chains over nested for loops]**
**Learning:** Re-implementing a `map` and `join` logic via a nested `for` loop with manual state tracking (like `first_obj`, `first_field` boolean flags) makes the code unnecessarily complex, harder to read, and error prone.
**Action:** Always prefer using idiomatic `.into_iter().map(...).collect()` pipelines combined with `.join(...)` when constructing comma-separated lists from collections.
