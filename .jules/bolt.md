**[QueryInsight String Lifetime Optimization]**
**Learning:** Returning strings by value using `clone()` in analytical helper structs like `QueryInsight` can cause unnecessary heap allocations, especially when the strings are just borrowed from a larger context (like an HTTP response). By introducing lifetime parameters (`<'a>`) and storing references (`&'a str`), we avoid these extra `.clone()` allocations and improve execution speed in hot paths without fighting the borrow checker.
**Action:** When creating structures to parse or analyze larger payloads (like JSON API responses), prefer borrowing strings using references and lifetimes (`<'a>`) instead of owning them as `String`s, provided the analysis doesn't outlive the underlying response payload.

**[QueryInsight String Lifetime Optimization]**
**Learning:** Returning strings by value using `clone()` in analytical helper structs like `QueryInsight` can cause unnecessary heap allocations, especially when the strings are just borrowed from a larger context (like an HTTP response). By introducing lifetime parameters (`<'a>`) and storing references (`&'a str`), we avoid these extra `.clone()` allocations and improve execution speed in hot paths without fighting the borrow checker.
**Action:** When creating structures to parse or analyze larger payloads (like JSON API responses), prefer borrowing strings using references and lifetimes (`<'a>`) instead of owning them as `String`s, provided the analysis doesn't outlive the underlying response payload.
**Refactoring String generators**
**Learning:** Returning `String` from code/diagram generators causes unnecessary allocations if the result is embedded into another large buffer.
**Action:** Expose `write_...(&mut String)` methods so callers can append directly to an existing buffer instead of allocating intermediate strings, but retain the original `to_...() -> String` implementation using `write_...` internally to prevent breaking existing tests.
**QueryInsight operations**
**Learning:** Using `String` for `operation_type` in `QueryInsight` causes many heap allocations via `.clone()` when parsing query plans.
**Action:** Add a lifetime parameter `<'a>` to `QueryInsight` and `QueryInsights` and use `&'a str` to borrow the `operation_type` string from the original `ExplainResponse`, avoiding clones.
