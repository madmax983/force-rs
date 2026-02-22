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

### 🟢 Acquitted (Fixed): `crates/force/src/api/composite/batch.rs`

**Module:** `crates::force::api::composite::batch`
**Severity:** 🟢 Acquitted (Fixed)
**Finding:** Missing validation for SObject names, IDs, empty batch, and batch size boundary.
**Evidence:**
1.  Mutation testing showed `BatchBuilder::execute` logic for batch size (> 25) was not fully tested (boundary conditions missed).
2.  Empty batch execution was not prevented, potentially sending invalid requests to Salesforce.
3.  Integration tests used invalid ID format ("invalid_id"), which passed only because client-side validation was missing.
**Resolution:**
1.  Implemented strict validation in `BatchBuilder` methods using `validator::validate_sobject_name` and `SalesforceId::new`.
2.  Added check for empty batch in `execute`.
3.  Added comprehensive unit tests covering validation failures, empty batch, and batch size limits (including boundary 25/26).
4.  Updated `tests/composite_batch.rs` to use syntactically valid Salesforce IDs, fixing the "Alibi Test" pattern.

### 🟢 Acquitted (Fixed): `crates/force/src/api/bulk/smart_ingest.rs`

**Module:** `crates::force::api::bulk::smart_ingest`
**Severity:** 🔴 Critical (Fixed)
**Finding:** "Dangling Job" bug and Ceremonial Tests.
**Evidence:**
1.  **Bug:** `execute_stream` returned early on upload failure, leaving the job in `Open` state on Salesforce. Confirmed by `test_smart_ingest_aborts_job_on_upload_failure` failure.
2.  **Weak Tests:** Initial mutation score was 0% (18/18 missed). Tests lacked `.expect(1)` (or explicit counts) on mocks, allowing them to pass even if logic was removed ("The Ceremony Test").
**Resolution:**
1.  **Fixed Bug:** Updated `execute_stream` to catch errors and explicitly call `abort` on the job.
2.  **Strengthened Tests:**
    *   Added `.expect(1)` (or explicit counts) to all mocks to ensure interactions occur.
    *   Added `test_smart_ingest_aborts_job_on_upload_failure` to verify the fix.
    *   Added `test_smart_ingest_empty_stream` for edge case coverage.
    *   Strengthened assertions in `test_smart_ingest_single_batch` to verify `JobInfo` content.
3.  **Result:** Mutation score improved significantly (critical logic now guarded).

### 🟢 Acquitted (Fixed): `crates/force/src/auth/token.rs`

**Module:** `crates::force::auth::token`
**Severity:** 🟡 Suspect (Fixed)
**Finding:** Inconsistent expiration logic and undocumented fallback.
**Evidence:**
1. `u64::MAX` resulted in a 3600-second expiration (via failure fallback), while `4_000_000_000` resulted in infinite expiration (via explicit cap).
2. `issued_at` parsing failure silently fell back to `Utc::now()` without test coverage.
**Resolution:**
1. Updated `expires_in` logic to treat all values > 3 billion (including `u64::MAX`) as infinite (None).
2. Added `test_access_token_from_response_invalid_issued_at` to verify/document the fallback behavior.
3. Added `crates/force/tests/token_audit.rs` as a regression suite for these edge cases.

### 🟢 Acquitted (Fixed): `crates/force/src/api/rest/soql.rs`

**Module:** `crates::force::api::rest::soql`
**Severity:** 🟢 Acquitted (Fixed)
**Finding:** Tests for error conditions and edge cases were missing.
**Evidence:**
1. `try_select` and `try_from` error paths were not tested.
2. `try_build` failure conditions (missing fields/sobject) were not tested.
3. `where_in` with empty list behavior was implicit.
4. `where_like` escaping was not explicitly verified.
**Resolution:**
1. Added `test_builder_try_methods_errors` and `test_build_errors` to verify validation logic.
2. Added `test_where_in_edge_cases` and `test_where_like` to verify correct query generation and escaping.
3. Added `test_limit_offset_only` and `test_order_independence` to ensure robust builder behavior.
