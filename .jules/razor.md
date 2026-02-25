## [Reduction]
**Bloat:** `crates/force/src/api/pub_sub.rs` and phantom features in `Cargo.toml`.
**Cut:** Deleted the file and removed `pub_sub`, `streaming`, `graphql`, `tooling`, `metadata`, `analytics`, `connect`, `apex_rest`, `ui`, `soap` features and associated dependencies.
**Saved:** 1 zombie file, ~10 unused features, 4 unused dependencies. Cognitive load reduced by removing fake capabilities.

## [Reduction]
**Bloat:** `DynamicSObjectBuilder` in `crates/force/src/types/sobject.rs`.
**Cut:** Removed the builder struct and implementation. Refactored tests to use `DynamicSObject::new` and `set_field`.
**Saved:** ~30 lines of code. Removed a "Factory Factory" pattern that added no value over direct mutation.

## [Reduction]
**Bloat:** `experimental` module and `nova` feature (Query Plan API).
**Cut:** Deleted `crates/force/src/experimental` directory, `explain.rs`, and removed `nova` feature.
**Saved:** 3 files (`scanner.rs`, `query_batch.rs`, `explain.rs`), ~500 lines of code. Removed speculative features and "zombie code".
