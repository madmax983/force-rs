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
- Replaced `assert!(result.is_ok())` with `.expect("...")` which guarantees the operation succeeded, bubbles up a helpful error message if it fails, and provides direct access to the `Ok` value.
