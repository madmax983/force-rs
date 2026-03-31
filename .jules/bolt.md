**⚡ Bolt: Refactored `compare_schemas` in `crates/force/src/experimental/schema_diff.rs`**
**Learning:** Instead of allocating a `HashMap` for both the old and new schema fields when calculating a diff, we can allocate just one for the `old_schema`. By iterating over `new_schema` fields directly and using `.remove()` on the `old_schema` map, we can avoid a second `HashMap` allocation and easily identify removed fields as those remaining in the map.
**Action:** When computing diffs between two lists, only index one of them into a `HashMap` to avoid unnecessary heap allocations.
**[Stack-Allocated Query Parameters in UI API]\n**Learning:** Dynamic heap allocations for tiny, short-lived collections like UI query parameters () represent an avoidable micro-overhead. \n**Action:** Use a fixed-size stack array and slice to completely eliminate heap allocation when parameter count is known and small.
**[Stack-Allocated Query Parameters in UI API]**
**Learning:** Dynamic heap allocations for tiny, short-lived collections like UI query parameters (`Vec<(&str, &str)>`) represent an avoidable micro-overhead.
**Action:** Use a fixed-size stack array and slice to completely eliminate heap allocation when parameter count is known and small.
**GraphqlErrorResponse Display Optimization**
**Learning:** When implementing `fmt::Display` for comma-separated (or otherwise joined) collections of strings, avoid calling `.collect()` into an intermediate `Vec` and then using `.join()`, as this triggers unnecessary heap allocations. Using `.collect()` on an ExactSizeIterator to build a `HashMap` correctly uses the size hint and pre-allocates under the hood, making a manual loop redundant.
**Action:** Iterate over the elements using `.enumerate()` and write directly to the `Formatter` (e.g., `if i > 0 { write!(f, "; ")?; } write!(f, "{}", item)?;`).
**Remove `.to_string()` on `utf8_percent_encode` result**
**Learning:** Calling `.to_string()` on the result of `percent_encoding::utf8_percent_encode()` before passing it into `format!` triggers an unnecessary intermediate heap allocation. The `PercentEncode` struct returned by this function implements `fmt::Display`, meaning it can be formatted directly.
**Action:** Pass `utf8_percent_encode` directly into `format!` arguments instead of creating temporary strings.
**Refactor map_field_type to write_field_type**
**Learning:** When repeatedly mapping enum values to string representations within a loop (e.g. generating SQL columns from FieldType), returning a new `String` (via `.to_string()` or `format!()`) creates excessive temporary heap allocations. Changing the function signature to accept a `&mut String` buffer and using `.write_str()` or `write!()` directly eliminates this overhead.
**Action:** For string-building operations on hot paths, pass a mutable buffer reference downward instead of allocating and returning new strings from helper functions.
