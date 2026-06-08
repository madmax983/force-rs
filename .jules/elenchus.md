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

**[Elenchus: force-pubsub::subscriber Test Quality Audit]**
**Module:** `crates/force-pubsub/src/subscriber.rs`
**Severity:** 🔴 Critical
**Finding:** `cargo mutants` exposed that capacity maths for `stream_channel_capacity`, retry delays based on `reconnect_count`, and `max_retries` comparison (`>`) were untested, masking false logic paths (like capping max delay on backoff or returning incorrect capacity).
**Evidence:** 7 surviving mutants in `stream_channel_capacity`, `handle_reconnect` backoff arguments, and `reconnect_count` condition.
**Recommendation:** Added explicit tests to verify max retries exhausted event states (checking exactly 2 Reconnected events for max 2 retries), backoff elapsed times via `Instant`, and `stream_channel_capacity` unit test to `crates/force-pubsub/tests/subscribe_events_tests.rs` and `crates/force-pubsub/src/subscriber.rs`.

**[Elenchus: force::api::rest_operation Test Quality Audit]**
**Module:** `crates/force/src/api/rest_operation.rs`
**Severity:** 🔴 Critical
**Finding:** Missing mutation test coverage in `upsert_with_retry_class` and `validate_query_input_len`. The `MAX_QUERY_INPUT_BYTES` threshold tests were weak, and the response limit calculation in `upsert` lacked explicit bound verification.
**Evidence:** 5+ surviving mutants around `100 * 1024 * 1024` limit calculation in `upsert` paths. Surviving `>=` mutant on `validate_query_input_len`.
**Recommendation:** Ensure exact boundary coverage in threshold validation methods (like EXACT max allowed size), and explicitly cover payload too large errors (`HttpError::PayloadTooLarge`) by setting up mocks with very large mock response data.
**[Elenchus: force-pubsub::subscriber handle_reconnect backoff math]**
**Module:** `crates/force-pubsub/src/subscriber.rs`
**Severity:** 🔴 Critical
**Finding:** The exponential backoff calculation `delay_for(*reconnect_count - 1)` had missing upper bound assertions in `test_subscribe_exhausts_retries_returns_error`. Mutants that replaced `-` with `+` or `/` survived because the test only asserted `elapsed >= 30ms`.
**Evidence:** 2 surviving mutants in `handle_reconnect` backoff index math.
**Recommendation:** Added an upper bound assertion (`elapsed < 50ms`) to properly verify that the correct delay values (10ms + 20ms = 30ms) were generated, catching the 60ms mutation.

**[Elenchus: force::api::rest_operation Exact Limit Boundary Tests]**
**Module:** `crates/force/src/api/rest_operation.rs`
**Severity:** 🔴 Critical
**Finding:** The `100 * 1024 * 1024` limit check in `read_capped_body_bytes` within `upsert` lacked an exact boundary test. `cargo mutants` exposed that mutating this boundary went un-caught.
**Evidence:** Surviving mutant on the limit calculation.
**Recommendation:** Added `test_upsert_payload_exactly_at_limit` to explicitly verify that exactly 100MB payloads are accepted.
