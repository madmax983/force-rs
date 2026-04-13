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

**Missing Optional Field Test Quality**
**Module:** `force::api::bulk::types`
**Severity:** 🔴 Critical
**Finding:** The test struct `Wrapper` in `test_deserialize_optional_string_or_number` lacked the `#[serde(default)]` attribute alongside `deserialize_with` for its optional field. Furthermore, there was no test asserting behavior against an entirely missing field (`{}`).
**Evidence:** `cargo mutants` indicated that `test_deserialize_optional_string_or_number` provided false confidence. Adding the missing field payload (`"{}"`) caused an immediate test panic (`called Result::unwrap() on an Err value: Error("missing field value")`), confirming the test was blind to this failure mode.
**Recommendation:** Add `#[serde(default)]` to the test `Wrapper` struct and implement an explicit assertion against an empty JSON object `"{}"` to ensure true mutation survivability and parser robustness.
