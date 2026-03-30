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

**[Enforce Idiomatic Test Unwrap]**
**Learning:** Re-implementing unwraps with `#![allow(clippy::unwrap_used)]` or manual expect inside tests suppresses useful lints and introduces repetitive failure messages.
**Action:** Consistently replace `unwrap()`/`expect()` in tests with the internal extension traits `.must()` and `.must_msg()` provided in `crate::test_support`. For `.unwrap_err()`, instead use explicit match destructing such as `let Err(err) = result else { panic!(...) }`.
**Field Validation Struct Extraction**\n**Learning:** Large character parsing loops implementing complex state machines become unreadable and hard to maintain when packed into a single function.\n**Action:** Extract long state machines into a clear structs with well-named internal state transitions (e.g. , ) to reduce cognitive load.
**Field Validation Struct Extraction**
**Learning:** Large character parsing loops implementing complex state machines become unreadable and hard to maintain when packed into a single function.
**Action:** Extract long state machines into a clear structs with well-named internal state transitions to reduce cognitive load.
**SObject Path Formatting**
**Learning:** Repetitive string formatting for API paths (like `/sobjects/{}` and `/sobjects/{}/{id}`) across multiple methods causes visual noise and copy-paste vulnerabilities.
**Action:** Centralize path construction into pure helper functions (e.g., `format_sobject_path(sobject: &str, id: Option<&str>) -> String`) to enforce DRY principles.

**[Centralize URL Path Construction]**
**Learning:** Re-implementing URL construction via intermediate base URLs and `format!` macros (e.g., `format!("{}/{}", self.base_url().await?, job_id)`) across bulk modules introduces duplicate logic and can be prone to slash/path errors.
**Action:** Always use the centralized `Session::resolve_url` directly with the full path suffix (e.g., `self.inner.resolve_url(&format!("jobs/ingest/{}", job_id))`).

**[Extract Complex God Functions]**
**Learning:** Functions handling multiple responsibilities like HTTP fetching, pagination, and data deserialization (e.g., `BulkQueryStream::next`) become overly long and complex, violating the single responsibility principle.
**Action:** Extract large blocks of logic into smaller, specifically named helper methods (e.g., `fetch_next_page`, `deserialize_csv`) to flatten the function structure and improve readability.

**[Future Not Send in Extracted Helpers]**
**Learning:** Extracting private asynchronous helper methods that hold `&mut self` across await points in generic structs can trigger `clippy::future_not_send` if the generic parameter `T` is not bound by `Send`.
**Action:** Apply `#[allow(clippy::future_not_send)]` to such internal helper methods to ensure they pass strict `clippy -D warnings` checks without altering the public trait bounds.
**Extract UrlEncodedWriter to Common Utils**
**Learning:** Both `batch.rs` and `graph.rs` duplicated the `UrlEncodedWriter` struct and its `std::fmt::Write` implementation for avoiding memory allocations during URL construction. This creates unnecessary DRY violations for a pure utility type.
**Action:** Extract `UrlEncodedWriter` into `crates/force/src/api/url_encoded_writer.rs` and re-use it across composite API implementations to keep the logic unified and DRY.
**[Extract is_retryable_error]**
**Learning:** Extracting complex inline `match` statements into dedicated helper methods significantly improves readability by flattening the pyramid of doom and providing a clear, descriptive name for the condition.
**Action:** Look for other complex inline match statements that can be extracted into helper methods.

**[Flatten is_retryable_error]**
**Learning:** Double `match` statements ("Pyramid of Doom") on Result/Error enum variants make simple logic unnecessarily nested and harder to quickly scan.
**Action:** Prefer `if let` guard clauses to handle outer wrappers, flattening the logic into a single un-nested match statement.

**[Flatten test error matching]**
**Learning:** Using `match` blocks to assert specific error variants in tests introduces unnecessary nesting (Pyramid of Doom).
**Action:** Prefer using `let Err(...) = result else { panic!(...) }` guard clauses to flatten test assertions.
