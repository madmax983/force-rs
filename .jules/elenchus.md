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

**Elenchus: SSRF Protection Missing Validation in resolve_next_records_url**
**Module:** `crates/force/src/api/rest_operation.rs`
**Severity:** 🔴 Critical
**Finding:** The `resolve_next_records_url` validation allowed SSRF bypasses via `@evil.com` overrides. Although tests previously existed, `cargo mutants` (or similar analysis) showed the previous logic naively prepended instances and then incorrectly checked origins or didn't validate certain inputs against the host. It also showed missing testing for scheme and host mismatches logic paths.
**Evidence:** The fix required converting to `url::Url::join()` to natively support resolving strings against the base `instance_url` according to RFC. Additionally, missing tests were added for host mismatches, scheme mismatches, port mismatches, and multiple mismatches using `#[test]`.
**Recommendation:** Already resolved. The `RestOperation` trait correctly uses `Url::join` now, and unit tests have been added for all variants.

**Elenchus: RestHandler path_prefix untested**
**Module:** `crates/force/src/api/rest/mod.rs`
**Severity:** 🟡 Suspect
**Finding:** The `path_prefix()` method implementation for `RestHandler<A>` was completely untested. `cargo mutants` exposed that modifying this method to return `"xyzzy"` did not fail any unit tests.
**Evidence:** `cargo mutants` run against `api::rest::mod.rs` with `api::rest::tests` targeted showed 1 MISSED mutant for `path_prefix()`.
**Recommendation:** Already resolved. A unit test `test_rest_handler_path_prefix` was added to verify `path_prefix()` correctly returns `""`.

**Elenchus: query MAX_QUERY_INPUT_BYTES limit untested**
**Module:** `crates/force/src/api/rest_operation.rs`
**Severity:** 🟡 Suspect
**Finding:** The helper method `validate_query_input_len()` inside `RestOperation` was not covered by a success scenario test. `cargo mutants` indicated that replacing its logic with `Ok(())` or flipping the `>` operator might go uncaught during standard test runs if only failure states are checked.
**Evidence:** `cargo mutants` revealed missed coverage in the `rest_operation.rs` file.
**Recommendation:** Already resolved. A unit test `test_validate_query_input_len_ok` was added to verify that exactly `MAX_QUERY_INPUT_BYTES` succeeds.
