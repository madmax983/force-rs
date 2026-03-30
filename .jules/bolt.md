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

**Eliminate intermediate string allocations in SQL schema generation**
**Learning:** Returning `String` from mapping functions like `map_field_type` by calling `.to_string()` or `format!()` causes unnecessary temporary heap allocations for every field when generating DDL.
**Action:** Refactor mapping functions that return Strings into writing functions that take `&mut String` (e.g., `write_field_type(out: &mut String)`) and use `.push_str()` or `write!()` directly to the output buffer to avoid these allocations entirely.
