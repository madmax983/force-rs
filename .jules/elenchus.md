# Elenchus Test Audit

## verdicts

**[Ceremonial Assertion]**
**Module:** `crates/force/src/client/builder.rs`
**Severity:** 🟡 Suspect
**Finding:** `test_builder_builds_client` asserts `Arc::strong_count(&client.inner) == 1`. This checks implementation details (reference counting) rather than observable behavior (e.g., correct configuration).
**Evidence:** `assert!(Arc::strong_count(&client.inner) == 1);`
**Recommendation:** Replace with assertions on `client.config()` or other public properties.

**[Tautological Mirroring & False Confidence]**
**Module:** `crates/force/src/types/query.rs`
**Severity:** 🔴 Critical
**Finding:** The `arbitrary_query_result` proptest strategy manually "fixes" inconsistencies (forcing `next_records_url=None` if `done=true`), effectively hiding invalid states from the tests.
**Evidence:**
```rust
if result.done {
    result.next_records_url = None;
}
```
This means `prop_done_implies_not_has_more` tests the generator, not the implementation.
**Recommendation:** Remove the manual fix-up in the generator to allow invalid states, and update tests to assert correct behavior even in invalid states (or fix the type to prevent invalid states).

**[Tautological Assertion]**
**Module:** `crates/force/src/types/query.rs`
**Severity:** 🟡 Suspect
**Finding:** `prop_has_more_implies_not_done` asserts `!result.done` when `result.has_more()` is true. Since `has_more()` is implemented as `!self.done`, this test is `assert_eq!(!done, !done)`.
**Evidence:** `prop_assert!(!result.is_done());`
**Recommendation:** Combine with consistency checks against `next_records_url` (e.g., `has_more() -> next_records_url.is_some()`), acknowledging that the type system currently allows inconsistency.

**[Missing Negative Test]**
**Module:** `crates/force/src/api/rest/query.rs`
**Severity:** 🟡 Suspect
**Finding:** No test covers the case where Salesforce returns `done: false` but omits `nextRecordsUrl` (which is technically invalid but possible).
**Evidence:** Integration tests only cover happy paths or 404s.
**Recommendation:** Add a test case with a mocked response having `done: false` and missing `nextRecordsUrl` to verify client behavior.
