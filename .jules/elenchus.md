# Elenchus Test Audit

## verdicts

**[Ceremonial Assertion]**
**Module:** `crates/force/src/client/builder.rs`
**Severity:** 🟢 Acquitted (Fixed)
**Finding:** Previously asserted on `Arc::strong_count`.
**Resolution:** Codebase has been updated; test now asserts on public configuration properties.

**[Tautological Mirroring & False Confidence]**
**Module:** `crates/force/src/types/query.rs`
**Severity:** 🟢 Acquitted (Fixed)
**Finding:** Generator previously "fixed" invalid states.
**Resolution:** Generator now produces raw random states, allowing tests to verify behavior under invalid conditions (e.g., `prop_done_implies_not_has_more` now correctly verifies safety despite invalid state).

**[Tautological Assertion]**
**Module:** `crates/force/src/types/query.rs`
**Severity:** 🟡 Suspect (Accepted)
**Finding:** `prop_has_more_is_inverse_of_done` asserts `!result.done` when `result.has_more()` is true.
**Evidence:** `prop_assert_eq!(result.has_more(), !result.is_done());`
**Resolution:** Accepted as a necessary verification of the API contract: `has_more()` *must* be the inverse of `done`, regardless of other internal state (like `next_records_url`). It ensures the `done` flag remains the source of truth.

**[Missing Negative Test]**
**Module:** `crates/force/src/api/rest/query.rs`
**Severity:** 🟢 Acquitted (Fixed)
**Finding:** No test for `done: false` but missing `nextRecordsUrl`.
**Resolution:** `test_query_pagination_missing_url` has been added to verify this scenario.

**[Ceremonial Assertion]**
**Module:** `crates/force/src/api/rest/crud.rs`
**Severity:** 🟢 Acquitted (Fixed)
**Finding:** `test_upsert_update` asserted `result.is_err() || result.is_ok()`, which is always true.
**Resolution:** Updated test to assert `matches!(result, Err(ForceError::NotImplemented(_)))`.

**[Untested Default Behavior]**
**Module:** `crates/force/src/http/mod.rs`
**Severity:** 🟡 Suspect (Fixed)
**Finding:** `parse_retry_after` relied on untested default (60s) for missing/invalid headers.
**Resolution:** Refactored to accept `&HeaderMap`, added 4 unit tests covering edge cases, and 2 integration tests verifying 429 defaults.

**[Time-Dependent Flakiness]**
**Module:** `crates/force/src/http/tests.rs`
**Severity:** 🟡 Suspect (Fixed)
**Finding:** `test_503_retries_with_exponential_backoff` relied on 1.5s real-time sleep, making it slow and potentially flaky.
**Resolution:** Refactored `HttpExecutor` to support configurable `base_backoff`, reducing test duration to 0.04s and ensuring determinism.

**[Missing Safe Method Classification]**
**Module:** `crates/force/src/http/mod.rs`
**Severity:** 🟢 Acquitted (Fixed)
**Finding:** `classify_request` excluded `TRACE` from retryable read operations.
**Resolution:** Added `TRACE` to `RequestRetryClass::Read`.
