# Elenchus's Journal

**The Oracle Problem in Bulk API Validation**
**Module:** `force::api::bulk::types` and `force::api::bulk::query`
**Severity:** 🔴 Critical
**Finding:** The `apiVersion` field serialization was missing the `default` attribute prior to `skip_serializing_if`, causing Serde deserialization to fail on JSON where `apiVersion` was missing entirely, but `cargo mutants` revealed that this failure mode was untested and masked.
**Evidence:** The `test_job_info_deserialization` tests failed when `cargo mutants` perturbed the `serde` payload structure to omit `apiVersion` because the custom deserializer `deserialize_optional_string_or_number` bypassed `Option`'s natural missing-field fallback without an explicit `default` tag.
**Recommendation:** Sentry must add `default` to any `Option<T>` field that also uses `deserialize_with`, and ensure a test exists that parses a minimal JSON object where every single optional field is missing, not just `null`.

**The Oracle Problem in Bulk API Validation**
**Module:** `force::api::bulk::types` and `force::api::bulk::query`
**Severity:** 🔴 Critical
**Finding:** The `apiVersion` field serialization was missing the `default` attribute prior to `skip_serializing_if`, causing Serde deserialization to fail on JSON where `apiVersion` was missing entirely, but `cargo mutants` revealed that this failure mode was untested and masked.
**Evidence:** The `test_job_info_deserialization` tests failed when `cargo mutants` perturbed the `serde` payload structure to omit `apiVersion` because the custom deserializer `deserialize_optional_string_or_number` bypassed `Option`'s natural missing-field fallback without an explicit `default` tag.
**Recommendation:** Sentry must add `default` to any `Option<T>` field that also uses `deserialize_with`, and ensure a test exists that parses a minimal JSON object where every single optional field is missing, not just `null`.


**[Elenchus: deserialize_optional_string_or_number test strengthening]**
**Module:** `crates/force/src/api/bulk/types.rs`
**Severity:** 🔴 Critical
**Finding:** The custom deserializer for `Option<String>` bypassed Serde's natural missing-field fallback when the field was completely absent from the JSON payload. `cargo mutants` exposed that the tests did not cover missing fields, empty strings, or arbitrary strings, providing false confidence.
**Evidence:** `cargo mutants` showed 3 surviving mutants in the implementation. Serde fails to deserialize completely missing fields unless the `#[serde(default)]` attribute is present on the `Option` field.
**Recommendation:** Add the explicit `#[serde(default)]` attribute to the test struct `Wrapper`. Enhance the tests to explicitly assert behavior for completely absent fields (`{}`), empty strings (`{"value": ""}`), and arbitrary strings (`{"value": "xyzzy"}`). Explicitly handle the `Null` variant internally in `StringOrNumber` to safely convert literal nulls to `None`.
**[Elenchus: QueryStream missing empty middle page fetch test]**
**Module:** `crates/force/src/api/bulk/query.rs`
**Severity:** 🔴 Critical
**Finding:** `QueryStream::next()` failed to loop properly when an empty middle page was fetched, immediately marking the stream as exhausted. `cargo mutants` exposed that this path was not tested at all.
**Evidence:** `cargo mutants` mutated `if self.records.is_empty()` replacing the condition causing early termination and no tests failed.
**Recommendation:** Wrap the fetching logic inside `QueryStream::next()` in a `loop` so that if an empty page is fetched but `next_locator` is still present, the stream fetches the next page. Add a test `test_query_results_fetch_csv_data_empty_middle_page` to simulate an empty middle page using `wiremock`.
**[Elenchus: QueryStream empty middle page early termination test]**
**Module:** `crates/force/src/api/bulk/query.rs`
**Severity:** 🔴 Critical
**Finding:** The `QueryStream::next()` loop conditionally relied on `self.records.is_empty() && self.next_locator.is_none()` for early termination. `cargo mutants` revealed that this conditional was weakly tested: mutating `&&` to `||` went uncaught by the existing test suite, meaning there was no guarantee the stream correctly handled empty middle pages where `next_locator` is *not* none.
**Evidence:** `cargo mutants -f crates/force/src/api/bulk/query.rs` flagged the `&& -> ||` mutation at line 189 as surviving.
**Recommendation:** Added `test_query_results_fetch_csv_data_empty_middle_page` using `wiremock` to simulate an exact sequence of fetching a populated page, an empty middle page with a locator, and a final populated page. This strongly tests the logic and successfully kills the mutation.
