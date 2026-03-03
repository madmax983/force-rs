## [Reduction]
**Bloat:** `crates/force/src/api/pub_sub.rs` and phantom features in `Cargo.toml`.
**Cut:** Deleted the file and removed `pub_sub`, `streaming`, `graphql`, `tooling`, `metadata`, `analytics`, `connect`, `apex_rest`, `ui`, `soap` features and associated dependencies.
**Saved:** 1 zombie file, ~10 unused features, 4 unused dependencies. Cognitive load reduced by removing fake capabilities.

## [Reduction]
**Bloat:** `DynamicSObjectBuilder` in `crates/force/src/types/sobject.rs`.
**Cut:** Removed the builder struct and implementation. Refactored tests to use `DynamicSObject::new` and `set_field`.
**Saved:** ~30 lines of code. Removed a "Factory Factory" pattern that added no value over direct mutation.

## [Reduction]
**Bloat:** `MustMsg` and `Must` traits in `test_support.rs`.
**Cut:** Deleted the traits and replaced their usages with the standard `.expect()` and `.unwrap()` methods respectively.
**Saved:** ~40 lines of boilerplate trait definitions and implementations. Removed custom test-specific terminology in favor of standard Rust idioms, reducing cognitive load for developers familiar with Rust.

## [Reduction]
**Bloat:** `ClientConfigBuilder` in `crates/force/src/config.rs`.
**Cut:** Deleted the "Factory Factory" pattern. Replaced usages with direct initialization using `ClientConfig { ...Default::default() }`.
**Saved:** ~60 lines of builder boilerplate and test code. Simplified the configuration process.
