**[QueryInsight String Lifetime Optimization]**
**Learning:** Returning strings by value using `clone()` in analytical helper structs like `QueryInsight` can cause unnecessary heap allocations, especially when the strings are just borrowed from a larger context (like an HTTP response). By introducing lifetime parameters (`<'a>`) and storing references (`&'a str`), we avoid these extra `.clone()` allocations and improve execution speed in hot paths without fighting the borrow checker.
**Action:** When creating structures to parse or analyze larger payloads (like JSON API responses), prefer borrowing strings using references and lifetimes (`<'a>`) instead of owning them as `String`s, provided the analysis doesn't outlive the underlying response payload.

**Eliminate intermediate string allocations in SQL schema generation**
**Learning:** Returning `String` from mapping functions like `map_field_type` by calling `.to_string()` or `format!()` causes unnecessary temporary heap allocations for every field when generating DDL.
**Action:** Refactor mapping functions that return Strings into writing functions that take `&mut String` (e.g., `write_field_type(out: &mut String)`) and use `.push_str()` or `write!()` directly to the output buffer to avoid these allocations entirely.
**Zero-Allocation JSON Hashing with `serde_json::to_writer`**
**Learning:** Hashing a `serde_json::Value` by first converting it to a `String` (e.g., `value.to_string().as_bytes()`) creates an expensive, unnecessary heap allocation equal to the size of the serialized JSON.
**Action:** Since most hashers (like `blake3::Hasher`) implement `std::io::Write`, use `serde_json::to_writer(&mut hasher, &value)` to stream the serialized bytes directly into the hasher, eliminating the intermediate string allocation entirely.
**[QueryInsight String Lifetime Optimization]**
**Learning:** Returning strings by value using `clone()` in analytical helper structs like `QueryInsight` can cause unnecessary heap allocations, especially when the strings are just borrowed from a larger context (like an HTTP response). By introducing lifetime parameters (`<'a>`) and storing references (`&'a str`), we avoid these extra `.clone()` allocations and improve execution speed in hot paths without fighting the borrow checker.
**Action:** When creating structures to parse or analyze larger payloads (like JSON API responses), prefer borrowing strings using references and lifetimes (`<'a>`) instead of owning them as `String`s, provided the analysis doesn't outlive the underlying response payload.
**Refactoring String generators**
**Learning:** Returning `String` from code/diagram generators causes unnecessary allocations if the result is embedded into another large buffer.
**Action:** Expose `write_...(&mut String)` methods so callers can append directly to an existing buffer instead of allocating intermediate strings, but retain the original `to_...() -> String` implementation using `write_...` internally to prevent breaking existing tests.
**QueryInsight operations**
**Learning:** Using `String` for `operation_type` in `QueryInsight` causes many heap allocations via `.clone()` when parsing query plans.
**Action:** Add a lifetime parameter `<'a>` to `QueryInsight` and `QueryInsights` and use `&'a str` to borrow the `operation_type` string from the original `ExplainResponse`, avoiding clones.
