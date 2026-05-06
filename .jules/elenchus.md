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

**[force-sync: task_queue coalesce logic hides null retries]**
**Module:** `crates/force-sync/src/store/pg/task_queue.rs`
**Severity:** 🔴 Critical
**Finding:** In `update_task_status_unguarded` and `update_task_status_guarded`, `next_attempt_at = coalesce($4::timestamptz, next_attempt_at)` is used to update the `next_attempt_at` field. `cargo mutants` exposed that if `$4` is `None` (like when calling `fail_task`), `coalesce` evaluates to the current `next_attempt_at` value rather than clearing it to `null`.
**Evidence:** 8 surviving mutants from modifying `PgStore` methods that call `update_task_status`, exposing lack of test coverage for clearing `next_attempt_at`.
**Recommendation:** Replace `coalesce($4::timestamptz, next_attempt_at)` with just `$4::timestamptz` so that `None` correctly translates to `null` in the database, allowing tasks to truly fail or complete without lingering retry times. Add a test to assert that `fail_task` clears `next_attempt_at`.
