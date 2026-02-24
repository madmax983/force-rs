## [Reduction]
**Bloat:** `crates/force/src/api/pub_sub.rs` and phantom features in `Cargo.toml`.
**Cut:** Deleted the file and removed `pub_sub`, `streaming`, `graphql`, `tooling`, `metadata`, `analytics`, `connect`, `apex_rest`, `ui`, `soap` features and associated dependencies.
**Saved:** 1 zombie file, ~10 unused features, 4 unused dependencies. Cognitive load reduced by removing fake capabilities.

## [Reduction]
**Bloat:** `DynamicSObjectBuilder` in `crates/force/src/types/sobject.rs`.
**Cut:** Removed the builder struct and implementation. Refactored tests to use `DynamicSObject::new` and `set_field`.
**Saved:** ~30 lines of code. Removed a "Factory Factory" pattern that added no value over direct mutation.

## [Reduction]
**Bloat:** `crates/force/src/experimental` (unused `FieldUsageScanner`) and `crates/force/src/api/rest/explain.rs` (developer tool `QueryPlan` API).
**Cut:** Deleted `experimental` module entirely. Deleted `explain.rs` and removed `nova` feature from `Cargo.toml`.
**Saved:** ~250 lines of code. Removed "nice-to-have" features that are not critical for a production runtime client.
