**Refactoring String generators**
**Learning:** Returning `String` from code/diagram generators causes unnecessary allocations if the result is embedded into another large buffer.
**Action:** Expose `write_...(&mut String)` methods so callers can append directly to an existing buffer instead of allocating intermediate strings, but retain the original `to_...() -> String` implementation using `write_...` internally to prevent breaking existing tests.
**Refactoring struct ownership to lifetimes**
**Learning:** Returning fully owned structs containing `String` and `Vec` from processing functions like `compare_schemas` creates massive allocation overhead due to `.clone()` calls.
**Action:** Introduce a lifetime parameter (e.g., `'a`) to the result struct, allowing it to borrow references (`&'a str`, `&'a FieldType`) from the input parameters. This eliminates heap allocations entirely while leveraging the borrow checker to guarantee safety without `unsafe`.
