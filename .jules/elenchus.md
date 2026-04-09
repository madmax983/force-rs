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


**The Oracle Problem in Bulk API Validation (Unkillable Mutants)**
**Module:** `force::api::bulk::types`
**Severity:** 🟡 Suspect
**Finding:** The custom deserializer `deserialize_optional_string_or_number` has three surviving mutants where returning `Ok(None)`, `Ok(Some(String::new()))`, and `Ok(Some("xyzzy".into()))` still allows tests to pass in mutation runs. This happens despite having explicit `assert_eq!` tests for those exact outputs.
**Evidence:** `cargo mutants -f crates/force/src/api/bulk/types.rs` repeatedly reports these 3 mutants as MISSED even after introducing robust missing-field test cases.
**Recommendation:** Sentry must investigate why the mutants are surviving. Is `cargo mutants` caching old binaries? Is there a macro obfuscating the mutation? Or perhaps the struct wrapper isn't adequately triggering a test failure if the returned option value changes randomly?
