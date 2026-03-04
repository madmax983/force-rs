## [Reduction]
**Bloat:** `crates/force/src/api/pub_sub.rs` and phantom features in `Cargo.toml`.
**Cut:** Deleted the file and removed `pub_sub`, `streaming`, `graphql`, `tooling`, `metadata`, `analytics`, `connect`, `apex_rest`, `ui`, `soap` features and associated dependencies.
**Saved:** 1 zombie file, ~10 unused features, 4 unused dependencies. Cognitive load reduced by removing fake capabilities.

## [Reduction]
**Bloat:** `DynamicSObjectBuilder` in `crates/force/src/types/sobject.rs`.
**Cut:** Removed the builder struct and implementation. Refactored tests to use `DynamicSObject::new` and `set_field`.
**Saved:** ~30 lines of code. Removed a "Factory Factory" pattern that added no value over direct mutation.

## [Reduction]
**Bloat:** The `WhereClause` enum in `crates/force/src/api/rest/soql.rs` which was an unnecessary abstraction over formatted SOQL strings. It added a layer of indirection for displaying conditions, making the code slightly more verbose and complex than necessary.
**Cut:** Removed the `WhereClause` enum entirely and its `Display` implementation. Replaced it with a simple `Vec<String>` in `SoqlQueryBuilder`, formatting the strings directly during construction.
**Saved:** ~25 lines of code. Removed a single-use enum abstraction, making the query builder more direct and easier to read.
