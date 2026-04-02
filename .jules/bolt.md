**Refactoring String generators**
**Learning:** Returning `String` from code/diagram generators causes unnecessary allocations if the result is embedded into another large buffer.
**Action:** Expose `write_...(&mut String)` methods so callers can append directly to an existing buffer instead of allocating intermediate strings, but retain the original `to_...() -> String` implementation using `write_...` internally to prevent breaking existing tests.
