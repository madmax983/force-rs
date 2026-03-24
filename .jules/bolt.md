**⚡ Bolt: Refactored `compare_schemas` in `crates/force/src/experimental/schema_diff.rs`**
**Learning:** Instead of allocating a `HashMap` for both the old and new schema fields when calculating a diff, we can allocate just one for the `old_schema`. By iterating over `new_schema` fields directly and using `.remove()` on the `old_schema` map, we can avoid a second `HashMap` allocation and easily identify removed fields as those remaining in the map.
**Action:** When computing diffs between two lists, only index one of them into a `HashMap` to avoid unnecessary heap allocations.

**Stack-allocated Query Parameters**
**Learning:** Using heap-allocated Vec for short-lived, small collections of query parameters creates unnecessary allocations, which degrade performance on hot paths.
**Action:** Replace `Vec<(&str, &str)>` with stack-allocated arrays and length trackers (e.g. `[("", ""); N]` and `slice[..len]`) to eliminate allocations.
