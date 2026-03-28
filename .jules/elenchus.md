# ⚔️ Elenchus Journal - Test Quality Audit

This journal records the findings of the Elenchus test audit.

## Verdicts & Patterns

| Verdict | Module | Severity | Finding |
| :--- | :--- | :--- | :--- |
| **Acquitted** | `crates/force/tests/auth_timeout.rs` | 🟢 Acquitted | Initial audit flagged "Weak Assertion", but verification confirmed assertions are strong (`e.is_timeout()`). |
| **Mirror Test** | `crates/force/tests/havoc_concurrency.rs` | 🟡 Suspect | The test defines a local `TokenManager` struct to test double-checked locking logic instead of testing the actual `force::auth::TokenManager`. While it uses `loom` to verify the *algorithm*, it does not verify the *implementation*. |
| **Acquitted** | `crates/force/tests/havoc_token_leak.rs` | 🟢 Acquitted | The test verifies both the side effect (attacker server not called) and the specific error cause (Security Error due to origin mismatch). |
| **Commended** | `crates/force/src/auth/token_manager.rs` | ⭐ Commended | Robust concurrency testing using `tokio::spawn`, atomic counters, and specific assertion of call counts. |
| **Strengthened** | `crates/force/tests/security_soql_injection.rs` | 🟢 Acquitted | Original tests were tautological. Added `test_soql_injection_real_world_vectors` to verify defense against actual attack payloads. |
| **Strengthened** | `crates/force/src/http/retry.rs` | 🟢 Acquitted | `classify_request` defaulted to `Mutation`. Added `test_classify_request_all_methods` to exhaustively verify all HTTP methods. |
| **Acquitted** | `crates/force/src/experimental/scanner.rs` | 🟢 Acquitted | Initial audit found missing tests for filtering, batching, and zero-division. Added comprehensive tests and verified with mutation testing (12 mutants caught). |
| **Acquitted** | `crates/force/src/experimental/query_batch.rs` | 🟢 Acquitted | Initial audit found missing coverage for `halt_on_error`, empty results, and partial failures. Added `test_query_batch_halt_on_error`, `test_query_batch_empty_results`, and `test_query_batch_mixed_results`. |
| **Acquitted** | `crates/force/src/api/composite/batch.rs` | 🟢 Acquitted | Initial audit found tautological encoding tests and fragile JSON assertions. Refactored to use hardcoded "golden" strings and structural JSON validation. |
| **Strengthened** | `crates/force/src/experimental/type_generator.rs` | 🔴 Critical | Replace `.contains()` checks with exact match (`assert_eq!`) against a golden string. Enhance case-conversion tests with comprehensive cases. Added missing coverage for `map_type`. |
| **Strengthened** | `crates/force/src/experimental/schema_analyzer.rs` | 🔴 Critical | Original test `test_schema_analyzer` provided only 6 total fields, meaning that the `total_fields / 10` division resulted in `0`. Tests did not effectively test logic. |

## Detailed Findings

### [Acquitted] `crates/force/src/experimental/query_batch.rs`

**Module:** `crates/force/src/experimental/query_batch.rs`
**Severity:** 🟢 Acquitted (was 🟡 Suspect)
**Finding:** Manual audit revealed that the `halt_on_error` configuration and error handling statistics (`ops_failed`) were not exercised by the existing happy-path tests.
**Evidence:**
- Existing tests only verified successful batch execution (`ops_succeeded`).
- No test case existed for `halt_on_error(true)`.
- No test case existed for empty query results.
**Resolution:** Added `test_query_batch_halt_on_error` to verify the flag propagates to the request. Added `test_query_batch_empty_results` to ensure graceful handling of empty sets. Added `test_query_batch_mixed_results` to verify `ops_failed` counting logic.
**Note:** `cargo mutants` reported 0 mutants for this file, likely due to feature flag complexity or tool limitations with this specific module structure.

### [Acquitted] `crates/force/src/experimental/scanner.rs`

**Module:** `crates/force/src/experimental/scanner.rs`
**Severity:** 🟢 Acquitted (was 🟡 Suspect)
**Finding:** `cargo mutants` revealed that the filtering logic (`is_scanable`) and division-by-zero protection were not covered by tests. Additionally, the batching logic (`chunks(20)`) was not exercised.
**Evidence:**
- Mutation `replace > with >=` survived (division by zero risk).
- Mutation `replace is_scanable -> bool with true` survived (filtering logic unused).
**Resolution:** Added `test_scan_with_unsupported_fields`, `test_scan_empty_table`, `test_scan_batching`, and `test_scan_api_errors`. Re-ran `cargo mutants` and confirmed 100% kill rate for viable mutants.

### [Acquitted] `crates/force/tests/auth_timeout.rs`

**Module:** `crates/force/tests/auth_timeout.rs`
**Severity:** 🟢 Acquitted (was 🔴 Critical)
**Finding:** Initial audit suspected weak `result.is_err()` assertions. Manual verification of the source code confirmed that the tests strictly assert `e.is_timeout()`.
**Evidence:**
```rust
match result {
    Err(ForceError::Http(HttpError::RequestFailed(e))) => {
        assert!(e.is_timeout(), "Expected timeout error, got: {e}");
    }
    // ...
}
```
**Resolution:** Verdict updated to Acquitted. No code changes required.

### [Mirror Test] `crates/force/tests/havoc_concurrency.rs`

**Module:** `crates/force/tests/havoc_concurrency.rs`
**Severity:** 🟡 Suspect
**Finding:** The test defines `struct TokenManager` inside the test module and tests that, rather than the production `force::auth::TokenManager`.
**Evidence:**
```rust
    // Simplified TokenManager logic for Loom testing
    struct TokenManager {
        token: RwLock<Option<String>>,
    }
```
**Recommendation:** Acknowledge the limitation (Loom requires specific types) but mark as Suspect because it doesn't test the shipping code. Added documentation to the test file explaining this constraint.

### [Acquitted] `crates/force/src/api/composite/batch.rs`

**Module:** `crates/force/src/api/composite/batch.rs`
**Severity:** 🟢 Acquitted (was 🟡 Suspect)

**Finding:**
- `test_batch_query_encoding` was previously tautological, mirroring the implementation's use of `url::form_urlencoded`. It proved only that the code was the code.
- `test_batch_request_serialization` used fragile string containment checks instead of structural JSON validation.

**Action Taken:**
- Refactored `test_batch_query_encoding` to assert against a hardcoded expected URL string, breaking the tautology.
- Added `test_batch_query_encoding_special_chars` to explicitly verify safe encoding of dangerous characters (`+`, `%`, `'`).
- Refactored `test_batch_request_serialization` to deserialize the output and verify specific fields, preventing regression where keys might be present but with wrong values or structure.

**Recommendation:**
- Maintain the practice of using hardcoded "golden" strings for serialization and encoding tests.
- Continue to prefer structural assertions over string matching for JSON.

### [Strengthened] `crates/force/src/api/rest/crud.rs`

**Module:** `crates/force/src/api/rest/crud.rs`
**Severity:** 🟡 Suspect
**Finding:** The `upsert` implementation contained a redundant `201` match arm that functionally shadowed a fallback `_ if response.status().is_success()` arm, leading to false confidence and surviving mutants. Furthermore, tests testing failure cases asserted only `assert!(result.is_err())`, missing the actual errors and potentially silencing entirely different failure reasons.
**Evidence:**
- `cargo mutants` reported that deleting the `201` arm or mutating the `.is_success()` check to `true` or `false` went uncaught by the test suite.
- Tests had weak `assert!(result.is_err())` validations, which are not assertions but prayers.
**Recommendation:**
- Removed the redundant `201` match arm in `upsert_with_retry_class`.
- Added `test_upsert_success_other_status` to test the `.is_success()` match arm with a 200 OK.
- Upgraded all `result.is_err()` assertions across tests to explicit `unwrap_err()` checks validating the specific expected `message` and `error_code`.
### [Acquitted] `crates/force/src/api/rest/query.rs`

**Module:** `crates/force/src/api/rest/query.rs`
**Severity:** 🟢 Acquitted (was 🟡 Suspect)
**Finding:** Manual audit and mutation testing (`cargo mutants`) revealed that the conditional `||` chain in `resolve_next_records_url` used for security checks against `next_records_url` wasn't exhaustively tested. Specifically, mutations modifying `||` to `&&` survived.
**Evidence:**
- Mutations `replace || with &&` at lines 123 and 126 in `resolve_next_records_url` were MISSED by previous test suites.
- The pre-existing tests (`test_query_more_security_check` and `test_query_more_security_check_credentials`) did not cover all logical variants to effectively test scheme, host, port, username, and password mismatches independently.
**Resolution:** Added `test_query_more_security_check_scheme_mismatch`, `test_query_more_security_check_port_mismatch`, and `test_query_more_security_check_username_mismatch` to exhaustively trigger each condition. Ran `cargo mutants` to confirm no logic mutants survived in the security check.

### [Strengthened] `crates/force/src/http/tests.rs` & `crates/force/src/api/bulk/csv.rs`

**Module:** `crates/force/src/http/tests.rs` and `crates/force/src/api/bulk/csv.rs`
**Severity:** 🟡 Suspect
**Finding:** Widespread use of "The Ceremony Test" pattern. Dozens of tests ended with `assert!(result.is_ok())` instead of directly unwrapping and explicitly asserting the inner values or error contexts. While `result.must()` was used in some places to get the inner value, the initial `assert!(result.is_ok())` was redundant, and worse, some tests didn't inspect the inner values at all.
**Evidence:**
- `grep "assert!(result.is_ok())"` returned numerous hits across both files.
- Tests like `test_serialize_empty_records` checked `is_ok` but didn't actually verify the output cleanly handled empty state via strong properties.
**Recommendation:**
- Replaced `assert!(result.is_ok())` with `.must()` which guarantees the operation succeeded, bubbles up a helpful error message if it fails, and provides direct access to the `Ok` value.

### [Strengthened] `crates/force/src/experimental/type_generator.rs`

**Module:** `crates/force/src/experimental/type_generator.rs`
**Severity:** 🔴 Critical
**Finding:** The `test_generate_struct` function uses `.contains()` to verify its output. This is fragile because it permits extra text and does not verify the exact structure of the output. The string conversion tests (`test_snake_case`, `test_pascal_case`) lacked examples covering multiple capitalization formats and punctuation. The `map_type` function was missing tests entirely.
**Evidence:** 16 mutants survived due to weak tests or complete absence of testing.
**Recommendation:** Replace `.contains()` checks with exact match (`assert_eq!`) against a golden string. Enhance case-conversion tests with comprehensive cases. Added missing coverage for `map_type`.

### [Strengthened] `crates/force/src/auth/jwt_bearer.rs`

**Module:** `crates/force/src/auth/jwt_bearer.rs`
**Severity:** 🟡 Suspect
**Finding:** The `test_generate_jwt` test only verifies the JWT structure (3 parts) but not its contents. Mutants modifying the expiration calculation (`now + 300`) survived.
**Evidence:**
- `cargo mutants` output showing `+` replaced with `-` and `*` in the `exp` claim calculation survived.
**Recommendation:** Strengthen the test to decode the base64 URL-encoded JWT payload into a JSON object and verify that the claims (`iss`, `sub`, `aud`) are correct, and specifically that the `exp` claim is approximately `now + 300`.

### [Strengthened] `crates/force/src/auth/token_manager.rs`

**Module:** `crates/force/src/auth/token_manager.rs`
**Severity:** 🟡 Suspect
**Finding:** The concurrent overwrite protection logic (`if current.issued_at() > arc_token.issued_at()`) was partially untested. While `force_refresh` had an overwrite test, `get_token_arc`'s hard expiration and soft expiration paths had identical untested conditions. Furthermore, the behavior when tokens have the exact same timestamp was untested, allowing mutants with `>=` and `==` operators to survive.
**Evidence:** `cargo mutants` reported 5 missed mutants related to `>` operators on lines 114, 146, and 226 in `TokenManager`.
**Recommendation:** Added `test_token_manager_hard_refresh_protects_against_overwrite`, `test_token_manager_soft_refresh_protects_against_overwrite`, and `test_token_manager_equality_overwrites` to explicitly test concurrent token injections and timestamp equality. This killed all 5 surviving mutants, achieving 100% mutation coverage for viable logic.

### [Acquitted] `crates/force/src/auth/jwt_bearer.rs` and `crates/force/src/auth/client_credentials.rs`

**Module:** `crates/force/src/auth/jwt_bearer.rs` and `crates/force/src/auth/client_credentials.rs`
**Severity:** 🟢 Acquitted
**Finding:** Mutation testing revealed that replacing `>` with `>=` in the `1024 * 1024` byte stream limit logic survived in `authenticate` methods for both `JwtBearerFlow` and `ClientCredentials`. Analysis showed this is an equivalent mutant; truncating exactly at 1MB or waiting for the next chunk to exceed 1MB results in the same final bounded string length, making the mutation practically unobservable without internal side channels.
**Evidence:** `cargo mutants` output showing exactly one missed mutant: `replace > with >= in <impl Authenticator for JwtBearerFlow>::authenticate`.
**Recommendation:** Acknowledge the limitation of mutation tools concerning equivalent mutants. No further testing or code change is needed for these specific truncation lines as they correctly enforce the 1MB cap.

### [Strengthened] `crates/force/src/api/composite/graph.rs`

**Module:** `crates/force/src/api/composite/graph.rs`
**Severity:** 🟡 Suspect
**Finding:** The `test_havoc_path_traversal` and `test_havoc_invalid_reference_id` tests used weak "Ceremony Test" assertions (`assert!(result.is_err())`). This provided false confidence because functions could fail for entirely unrelated reasons, or they could falsely pass tests if a mutant accidentally converted validation logic to return `Ok(())` while another validation logic step failed.
**Evidence:**
- `cargo mutants` reported that multiple mutations within `validate_reference_id` (e.g., `replace || with &&`, `replace validate_reference_id -> Result<()> with Ok(())`) and `validate_graph_id` went uncaught by the test suite.
**Recommendation:** Replaced `assert!(result.is_err())` with explicit unwrapping and assertion of the returned error context to guarantee that the test fails if the *specific* validation fails, ensuring it correctly catches logic regressions.
### [Strengthened] `crates/force/src/experimental/schema_analyzer.rs`

**Module:** `crates/force/src/experimental/schema_analyzer.rs`
**Severity:** 🔴 Critical
**Finding:** The original test `test_schema_analyzer` provided only 6 total fields, meaning that the `total_fields / 10` division resulted in `0`. This made the test blind to mutants replacing division `/` with multiplication `*` (`6 * 10 = 60`), which caused false confidence. Additionally, the existing test inputs did not robustly exercise all `+` and `*` operators in the `complexity_score` calculation.
**Evidence:** `cargo mutants` revealed that multiple logic mutants in `SchemaAnalyzer::analyze` replacing `*` with `+` or `/`, and `/` with `*` survived, indicating that the test suite was insufficiently sensitive to mathematical logic errors and edge cases in the scoring heuristic.
**Recommendation:** Added `test_schema_analyzer_complexity_math` with exactly 12 fields (so `12 / 10 = 1`) and non-zero counts for custom, formula, and relationship fields to ensure all mathematical operations `*`, `/`, and `+` produce meaningful, non-identity/non-zero outcomes that effectively kill the mathematical mutants.

### [Strengthened] `crates/force/src/api/rest/search.rs`

**Module:** `crates/force/src/api/rest/search.rs`
**Severity:** 🟡 Suspect
**Finding:** The `SearchQueryBuilder::in_sidebar_fields` method lacked dedicated tests to verify that it correctly modified the `search_scope` property and correctly formatted the resultant SOSL query. This allowed a mutant substituting the body of the function with `Default::default()` to survive unnoticed.
**Evidence:**
- `cargo mutants` output showing a single surviving mutant: `replace SearchQueryBuilder::in_sidebar_fields -> Self with Default::default()`.
**Recommendation:** Added `test_search_query_builder_sidebar_fields` to construct a query explicitly invoking `.in_sidebar_fields()` and verified its output matched the expected string exactly, killing the remaining mutant and closing the testing gap.

### [Strengthened] `assert!(result.is_err())` Eradication Across `crates/force/src`

**Module:** `crates/force/src/...`
**Severity:** 🟡 Suspect
**Finding:** Widespread use of `assert!(result.is_err())` ("The Ceremony Test" pattern) provided false confidence. As Elenchus dictates: `assert!(result.is_err())` is not an assertion — it is a prayer. It hides the underlying error variants and survives mutation because it permits tests to pass if the function fails for completely unrelated reasons.
**Evidence:** Found over 40 occurrences across `scanner.rs`, `soql.rs`, `search.rs`, `limits.rs`, `csv.rs`, `ingest.rs`, `smart_ingest.rs`, `query.rs`, `error.rs`, `token_manager.rs`, `client_credentials.rs`, `jwt_bearer.rs`, `token.rs`, `salesforce_id.rs`, and `sobject.rs`.
**Recommendation:** Refactored tests to explicitly unwrap the error using `let Err(variant) = result else { panic!("...") }` and assertions to ensure tests only pass if they fail exactly as intended.

### [Strengthened] `crates/force/src/experimental/schema_analyzer.rs`

**Module:** `crates/force/src/experimental/schema_analyzer.rs`
**Severity:** 🔴 Critical
**Finding:** The tests in `test_schema_analyzer` and `test_schema_analyzer_complexity_math` did not effectively exercise mathematical logic limits (`total_fields / 10`) yielding non-zero values or permutations of conditions (`&&` to `||`).
**Evidence:** `cargo mutants` revealed that mutants modifying mathematical operators and boolean logic went uncaught.
**Recommendation:** Refactored tests by introducing `test_schema_analyzer_exhaustive_mutants` to strictly define 21 distinct fields. This ensures all mathematical formulas (e.g. division resulting in non-zero values) and combinations of `custom`, `nillable`, and `reference` logic are fully exercised.


### [Strengthened] `assert!(result.is_err())` Eradication Across `crates/force/src/api/` and `crates/force-pubsub/`

**Module:** `crates/force/src/api/` and `crates/force-pubsub/`
**Severity:** 🟡 Suspect
**Finding:** "The Ceremony Test" pattern `assert!(result.is_err())` was found in multiple UI, tooling, GraphQL, and pubsub tests. This provides false confidence because tests could falsely pass if the function fails for completely unrelated reasons.
**Evidence:** Found over 30 occurrences across `graphql/mod.rs`, `ui/favorites.rs`, `ui/lookups.rs`, `ui/list_views.rs`, `ui/actions.rs`, `ui/layouts.rs`, `ui/object_info.rs`, `ui/records.rs`, `tooling/execute_anonymous.rs`, `tooling/run_tests.rs`, `tooling/completions.rs`, `force-pubsub/src/codec.rs`, `force-pubsub/src/schema_cache.rs`, and `force-pubsub/tests/handler_tests.rs`.
**Recommendation:** Refactored tests to explicitly unwrap the error using `let Err(err) = result else { panic!("Expected an error"); };` and assertions to ensure tests only pass if they fail exactly as intended.

### [Strengthened] `crates/force/src/experimental/schema_analyzer.rs`

**Module:** `crates/force/src/experimental/schema_analyzer.rs`
**Severity:** 🔴 Critical
**Finding:** 30 mutants survived the `test_schema_analyzer_exhaustive_mutants` test due to mathematical overlaps. Because certain counts equaled each other or evaluated equally under different operators (e.g. `2*2=4` vs `2+2=4`, `2/10=0` vs `2%10=0`), `cargo mutants` demonstrated the test was blind to logic flaws and mathematical operator changes (`*` swapped with `/`, `+`, `-`, and `=` swapped with `!=`).
**Evidence:** `cargo mutants -d crates/force -f crates/force/src/experimental/schema_analyzer.rs -F schema` reported 30 missed logic mutants spanning mathematical replacements and boolean condition flips (`&&` to `||`, `!x` to `x`).
**Recommendation:** Completely rewrote the mock SObject definition within `test_schema_analyzer_exhaustive_mutants`. Set field counts to exact, non-overlapping values (`custom=9`, `formula=2`, `reference=4`, `required=3`, `total=21`, `standard=12`) that break all mathematical tautologies (e.g. `3*2=6` vs `3+2=5`). Tested every permutation of the `!nillable && !defaulted_on_create && name != "Id"` conditional to ensure that mutants mutating boolean operators (`&&`, `!`) are successfully killed. Verified 0 logic mutants survive.

### [Strengthened] `crates/force/src/experimental/data_faker.rs`

**Module:** `crates/force/src/experimental/data_faker.rs`
**Severity:** 🟡 Suspect
**Finding:** The `FieldType::Picklist`, `FieldType::Multipicklist`, and `FieldType::Combobox` fallback logic (checking active values vs. first values vs. defaults) and ignored types logic were completely untested. `cargo mutants` reported 0 missed mutants because it currently does not reliably mutate complex, nested `if let Some` branches inside `match` arms unless they involve boolean logic or numeric return values. This created false confidence in the test coverage.
**Evidence:** `cargo mutants` reported 100% kill rate, yet manual review showed several branches in the `generate_mock_record` match statement had zero coverage.
**Recommendation:** Added `test_generate_mock_record_picklists` to exhaustively test picklist behavior based on `active` status and fallback values, and to ensure skipped types (`Base64`, `Location`, `Address`, `Datacategorygroupreference`) are safely ignored.

### [Strengthened] `crates/force/examples/bulk_query.rs`

**Module:** `crates/force/examples/bulk_query.rs`
**Severity:** 🟢 Acquitted
**Finding:** The `stream.next().await` code block in the `README.md` and `bulk_query.rs` example caused confusion and failed to compile when copied because `BulkQueryStream::next()` is an inherent method, but the code comment explicitly referred to needing the `futures::StreamExt` trait. The lack of standard `futures::StreamExt` implementations made the provided examples difficult to adapt for standard async combinators.
**Evidence:** User reported compilation failure (`ECHO_ISSUE.md`) indicating `next` method was missing and expecting `futures::StreamExt`.
**Recommendation:** Refactored the `README.md` and `bulk_query.rs` examples to explicitly call `.into_stream()` to convert `BulkQueryStream` into a standard `futures::Stream`, added the `use futures::StreamExt;` import, wrapped the result via `std::pin::pin!`, and updated the `while let Some` loop to handle the resulting `Option<Result<T>>`. Tests and examples now successfully compile.
