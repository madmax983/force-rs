# Elenchus Journal - Verdicts & Patterns

**[Ceremony Test]**
**Module:** crates::force::client::mod
**Severity:** 🟡 Suspect
**Finding:** `test_builder_creates_noauth_state` was a tautological "ceremony test". It assigned the result of `builder()` to a variable without checking its type or value, meaning the test would pass regardless of the return type (as long as it compiled).
**Evidence:** The test body was `let _builder = builder();`. No assertions.
**Action Taken:** Strengthened the test by adding an explicit type annotation: `let _builder: ForceClientBuilder<NoAuth> = builder();`. This ensures the API contract is enforced at compile-time.
**Status:** Fixed.

**[Missing Error Coverage]**
**Module:** crates::force::api::bulk::query
**Severity:** 🟡 Suspect
**Finding:** `BulkQueryStream` implementation handles CSV deserialization errors by returning a 500 error, but there was no test verifying this behavior when the API returns malformed CSV data.
**Evidence:** Code inspection showed `map_err` to `HttpError::StatusError`, but no test case exercised this path.
**Action Taken:** Added `test_query_results_malformed_csv` which mocks a response with a missing column and asserts that `stream.next()` returns an error.
**Status:** Fixed.

**[Exemplary Testing]**
**Module:** crates::force::api::bulk::csv
**Severity:** ⭐ Commended
**Finding:** The module uses `proptest` for property-based testing of CSV serialization and deserialization, covering round-trips, batching logic, and large datasets. This is high-quality testing that goes beyond simple example-based tests.
**Recommendation:** This pattern should be adopted for other complex data processing modules.

**[Comprehensive Integration]**
**Module:** crates::force::http::tests
**Severity:** 🟢 Acquitted
**Finding:** The HTTP layer has a comprehensive suite of integration tests (using `wiremock`) covering success paths, error handling (401 refresh, 429 retry-after, 503 backoff), and edge cases like double-401 failures and non-idempotent mutation handling.
**Recommendation:** Maintain this standard for new HTTP-related features.
