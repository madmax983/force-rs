# Elenchus's Journal ⚔️

This file records the cross-examination of the test suite.

## Philosophy

A test that cannot fail is not a test — it is a decoration.
Coverage is a liar's metric. Mutation score is the polygraph.
The most dangerous test is one that passes for the wrong reason.

## Verdicts & Patterns

### 🟢 Acquitted (Fixed): `crates/force/src/api/rest/query.rs`

**Module:** `crates::force::api::rest::query`
**Severity:** 🟢 Acquitted (Fixed)
**Finding:** Missing coverage for absolute `nextRecordsUrl`.
**Evidence:** The implementation handles `next_records_url.starts_with("http")`, but no test exercises this branch. If Salesforce changes behavior or the logic regresses, pagination could fail silently or with confusing errors.
**Resolution:** Added `test_query_more_absolute_url` integration test.

### 🟢 Acquitted (Fixed): `crates/force/src/api/rest/query_stream.rs`

**Module:** `crates::force::api::rest::query_stream`
**Severity:** 🟢 Acquitted (Fixed)
**Finding:** Missing coverage for pagination failures and empty middle pages.
**Evidence:**
1.  No test verifies that `stream.next()` returns the buffered records from the first page before failing on the second page fetch.
2.  No test verifies that `stream.next()` correctly handles an empty page (0 records) when `done: false`, ensuring it automatically fetches the next page instead of yielding `None` prematurely.
**Resolution:** Added `test_query_stream_pagination_error` and `test_query_stream_empty_middle_page`.

### 🟢 Acquitted (Fixed): `crates/force/src/api/rest/search.rs`

**Module:** `crates::force::api::rest::search`
**Severity:** 🟢 Acquitted (Fixed)
**Finding:** Strong tests for builder logic and SOSL injection prevention.
**Evidence:**
1.  `test_search_query_builder_escaping` explicitly tests special character escaping.
2.  `validate_field_syntax` is well-tested for balanced parentheses and quotes.
**Resolution:** Added one edge case for escaped quotes within string literals (`'O\'Reilly'`).

### 🟢 Acquitted (Fixed): `crates/force/src/types/token.rs`

**Module:** `crates::force::types::token`
**Severity:** 🟢 Acquitted (Fixed)
**Finding:** Minor boundary condition in `expires_in` cap.
**Evidence:** Mutation `replace > with >=` survived, indicating that the exact boundary value (3 billion) was not tested.
**Resolution:** Added `test_access_token_expires_in_cap_boundary` to verify behavior at the boundary.

### 🟢 Acquitted (Fixed): `crates/force/src/http/mod.rs` & `tests.rs`

**Module:** `crates::force::http`
**Severity:** 🟢 Acquitted (Fixed)
**Finding:** Weak assertions in telemetry tests and missing unit tests for `RequestRetryClass`.
**Evidence:**
1.  Mutant `replace RequestRetryClass::as_str -> ""` survived because integration tests did not assert on `request_class`.
2.  Unit tests for `RequestRetryClass` were missing.
**Resolution:**
1.  Added `test_request_retry_class_as_str` unit test.
2.  Strengthened `test_telemetry_hooks_capture_retry_and_completion` integration test to assert `request_class`.
