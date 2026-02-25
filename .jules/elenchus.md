# ⚔️ Elenchus Journal - Test Quality Audit

This journal records the findings of the Elenchus test audit.

## Verdicts & Patterns

| Verdict | Module | Severity | Finding |
| :--- | :--- | :--- | :--- |
| **Weak Assertion** | `crates/force/tests/auth_timeout.rs` | 🔴 Critical | The test asserts `result.is_err()` without checking the error type. This provides false confidence as it would pass on *any* error (e.g., DNS failure, 404), not just the intended timeout. |
| **Mirror Test** | `crates/force/tests/havoc_concurrency.rs` | 🟡 Suspect | The test defines a local `TokenManager` struct to test double-checked locking logic instead of testing the actual `force::auth::TokenManager`. While it uses `loom` to verify the *algorithm*, it does not verify the *implementation*. |
| **Acquitted** | `crates/force/tests/havoc_token_leak.rs` | 🟢 Acquitted | The test verifies both the side effect (attacker server not called) and the specific error cause (Security Error due to origin mismatch). |
| **Commended** | `crates/force/src/auth/token_manager.rs` | ⭐ Commended | Robust concurrency testing using `tokio::spawn`, atomic counters, and specific assertion of call counts. |
| **Suspect** | `crates/force/tests/security_soql_injection.rs` | 🟡 Suspect | The test mirrors the implementation of `escape_soql`. While valuable as a regression guard, it is tautological in nature. |
| **Acquitted** | `crates/force/src/experimental/scanner.rs` | 🟢 Acquitted | Initial audit found missing tests for filtering, batching, and zero-division. Added comprehensive tests and verified with mutation testing (12 mutants caught). |
| **Acquitted** | `crates/force/src/api/composite/batch.rs` | 🟢 Acquitted | Initial audit found tautological encoding tests and fragile JSON assertions. Refactored to use hardcoded "golden" strings and structural JSON validation. |

## Detailed Findings

### [Acquitted] `crates/force/src/experimental/scanner.rs`

**Module:** `crates/force/src/experimental/scanner.rs`
**Severity:** 🟢 Acquitted (was 🟡 Suspect)
**Finding:** `cargo mutants` revealed that the filtering logic (`is_scanable`) and division-by-zero protection were not covered by tests. Additionally, the batching logic (`chunks(20)`) was not exercised.
**Evidence:**
- Mutation `replace > with >=` survived (division by zero risk).
- Mutation `replace is_scanable -> bool with true` survived (filtering logic unused).
**Resolution:** Added `test_scan_with_unsupported_fields`, `test_scan_empty_table`, `test_scan_batching`, and `test_scan_api_errors`. Re-ran `cargo mutants` and confirmed 100% kill rate for viable mutants.

### [Weak Assertion] `crates/force/tests/auth_timeout.rs`

**Module:** `crates/force/tests/auth_timeout.rs`
**Severity:** 🔴 Critical
**Finding:** The test `test_client_credentials_timeout` (and others in the file) asserts `result.is_err()` to verify a timeout.
**Evidence:**
```rust
    let result = auth.authenticate().await;
    // Should fail with timeout
    assert!(result.is_err());
```
**Recommendation:** Modify the test to inspect the error and verify it is indeed a timeout error.

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
**Recommendation:** Acknowledge the limitation (Loom requires specific types) but mark as Suspect because it doesn't test the shipping code. Ideally, refactor production code to be generic over the lock type or use `cfg` to allow Loom testing, but for now, rely on `crates/force/src/auth/token_manager.rs` tests.

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
