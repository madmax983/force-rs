# Sentry's Journal

## [Exponential Backoff Overflow]
**Learning:** `u128::pow` panics on overflow, and simple multiplication can also overflow. In `exponential_backoff`, a retry loop with a large attempt count (e.g. 200) caused a panic because $500 * 2^{200}$ exceeds `u128::MAX`. Even though the result was intended to be capped at 30 seconds, the intermediate calculation overflowed before the cap was applied.
**Action:** Always check input bounds before exponentiation or use saturating/checked arithmetic, especially when inputs are driven by loops or configuration. Use early returns for capped values to avoid unnecessary large number calculations.

## [Double-Prefixing URLs]
**Learning:** `query_more` assumed `nextRecordsUrl` was always relative, unconditionally prepending the instance URL. This failed when Salesforce (or a mock) returned an absolute URL, resulting in `https://instance...https://instance...`.
**Action:** Always check if a URL is already absolute (e.g., `starts_with("http")`) before prepending a base URL. Use `reqwest::Url::parse` or manual checks to handle both cases gracefully.

## [Backoff Cap Logic]
**Learning:** `exponential_backoff` logic `min(calculated, MAX)` unconditionally capped the delay at 30s, ignoring the user-configured `base_backoff`. If a user set `base_backoff` to 60s, the client would still retry every 30s.
**Action:** Ensure that capping logic respects the user's minimum configuration. `max(base, MAX)` ensures the delay is never shorter than the base backoff.

## [Timestamp Precision Loss]
**Learning:** `parse_issued_at` used integer division by 1000 to convert milliseconds to seconds, discarding sub-second precision. This could lead to incorrect token expiration calculations (off by up to 1 second).
**Action:** Use `DateTime::from_timestamp_millis` or similar high-precision constructors when parsing timestamps to preserve fidelity.

## [Silent Data Loss on Zero Batch Size]
**Learning:** Functions that batch data (like `process_csv_batches`) must explicitly reject `batch_size: 0`. The iterator logic `0..0` creates an empty range, leading to immediate termination (`None`) which was interpreted as "successful completion" rather than "invalid configuration". This caused silent data loss where records were simply ignored.
**Action:** Always validate configuration parameters like `batch_size`, `concurrency`, etc., against 0 if 0 is not a valid state (like "infinite"). Use `NonZeroUsize` where applicable or explicit checks.

## [BatchBuilder URL Encoding Blindspot]
**Learning:** `BatchBuilder::add_request` accepts raw `&str` URLs and includes them directly in the composite request body without validation or encoding. This differs from `query()` which handles encoding. Users might pass `query?q=Select Id From Account` with spaces, which Salesforce might reject as invalid JSON/URL.
**Action:** When adding "raw" methods to builders, document whether inputs are treated as raw or encoded, and add tests verifying this behavior to prevent accidental regressions or assumptions.

## [Retry Logic Gap]
**Learning:** The documentation claimed network errors were retried, but the implementation only retried 503s. Code analysis revealed `execute_attempt` returned `Err` which bypassed the retry loop.
**Action:** Always verify "documented behavior" with a test case, especially for error handling paths which are often neglected.

## [Wiremock Delays]
**Learning:** `wiremock`'s `set_delay` is a powerful way to simulate timeouts without relying on flaky `sleep` or external networks.
**Action:** Use `set_delay` for timeout testing instead of `tokio::time::sleep` in test logic.
**Doc Tests for Public API**\n**Learning:** Added doc tests to  methods to improve coverage and provide usage examples.\n**Action:** Use doc tests for public API methods to ensure they compile and serve as documentation.
**Doc Tests for Public API**
**Learning:** Added doc tests to SoqlQueryBuilder methods to improve coverage and provide usage examples.
**Action:** Use doc tests for public API methods to ensure they compile and serve as documentation.
## [Graph API Validation Robustness]
**Learning:** Added exhaustive table-driven tests for `validate_reference_id` and `validate_graph_id`.
**Action:** Always test boundary validation helpers against a full table of invalid characters instead of relying on narrow single-character cases.

**[Composite Graph ID and Reference Validation]**
**Learning:** `test_havoc_path_traversal` and `test_havoc_invalid_reference_id` failed because `validate_graph_id` and `validate_reference_id` in the Composite Graph API were not properly validating their inputs against path traversal and invalid characters.
**Action:** When inspecting API endpoints that take parameters like `id` and `reference_id`, always verify that the validation logic rejects path traversal elements (`/`, `..`, `\`, `?`) and invalid character sequences to prevent vulnerabilities. Ensure explicit unit tests exist for all internal validation helpers.
**2024-03-26 - [Unenforced unwrap_or_panic caught by missing test]
**Learning:** Found a missing test gap for a custom `unwrap_or_panic` implementation on `SoqlQueryBuilder::select` handling invalid characters.
**Action:** Check that untested execution paths using helper methods like `unwrap_or_panic` or `unwrap_or_else` contain proper explicit tests using `#[should_panic]`. Always append `>>` to memory.
>> **2024-03-27 - [SearchQueryBuilder Missing Panic Tests]**
**Learning:** The `SearchQueryBuilder::build` and `SearchQueryBuilder::returning` methods implement custom `unwrap_or_panic` error handling similar to `SoqlQueryBuilder`. However, these paths were untested, meaning `unwrap_or_else(|e| panic!(...))` code was executed without test coverage verifying the panic behaviour/messaging on invalid states.
**Action:** When encountering a builder pattern containing `try_x` (Result) alongside `x` (panicking) methods wrapping `unwrap_or_panic()`, always verify that both the success `Result` path and the panic wrapper path are explicitly tested with `#[should_panic]` to maintain safety guarantees.
**Testing Inner Wrapper Panic Guards**
**Learning:** We rely heavily on centralizing wrapper helpers (`unwrap_or_panic`) within Builders to map underlying validation error outputs (`try_` functions) into unified panic strings for invalid API usage.
**Action:** Always make sure the shared panic formatting utility functions are covered by at least one explicit test using `#[should_panic(expected = "...")]` to guarantee consistency inside these developer convenience paths and avoid obscure regressions during string mapping refactoring.
**QueryStream and QueryIterator Edge Cases**
**Learning:** `QueryStream` and `QueryIterator` have complex state machines around `done`, `exhausted`, and empty pages, which were untested. `QueryResult` can theoretically represent invalid states (e.g. `done: true` but `next_records_url: Some(...)`) and the iterator needs to handle them safely.
**Action:** Added dedicated tests to verify safe iteration through empty middle pages, graceful handling of error propagation within streams, and proper exhaustion checks to prevent infinite loops.

**GraphQL Empty Error Branch**
**Learning:** The Salesforce GraphQL API can sometimes return a response with `data: null` and `errors: []` (an empty errors array). While testing complex `match` statements handling optional data and error structures, ensuring that fallback/catch-all branches exist is critical because if the `if !errors.is_empty()` guard is falsely assumed or missed, it will fail to drop down into the generic fallback error.
**Action:** When handling arrays of external errors or events that might be unexpectedly empty, ensure dedicated mock tests simulate the `[]` state alongside the standard `Some` and `None` states to ensure logic routing handles all variants correctly.

**[Avoid `#![allow(clippy::unwrap_used)]` in Test Modules]**
**Learning:** Using `#![allow(clippy::unwrap_used)]` to suppress unwrap warnings in test modules bypasses the project's custom `Must` trait (`.must()`), which provides better diagnostic messages on panics.
**Action:** Always use the `.must()` or `.must_msg()` extensions from `crate::test_support::Must` instead of `unwrap()` in tests, and avoid suppressing the `clippy::unwrap_used` lint.
**2024-03-28 - [BuilderUnwrapExt Missing Panic Tests]**
**Learning:** `BuilderUnwrapExt`'s `unwrap_or_panic` method was completely untested, meaning the shared panic formatting utility was unverified.
**Action:** Always make sure central/shared panic wrapper extensions (like `BuilderUnwrapExt::unwrap_or_panic`) are covered by explicit `#[should_panic]` unit tests within their own module to guarantee consistency across all usage sites.
