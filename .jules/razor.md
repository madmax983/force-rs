## [Reduction]
**Bloat:** `crates/force/src/api/pub_sub.rs` and phantom features in `Cargo.toml`.
**Cut:** Deleted the file and removed `pub_sub`, `streaming`, `graphql`, `tooling`, `metadata`, `analytics`, `connect`, `apex_rest`, `ui`, `soap` features and associated dependencies.
**Saved:** 1 zombie file, ~10 unused features, 4 unused dependencies. Cognitive load reduced by removing fake capabilities.

## [Reduction]
**Bloat:** `DynamicSObjectBuilder` in `crates/force/src/types/sobject.rs`.
**Cut:** Removed the builder struct and implementation. Refactored tests to use `DynamicSObject::new` and `set_field`.
**Saved:** ~30 lines of code. Removed a "Factory Factory" pattern that added no value over direct mutation.

## [Reduction]
**Bloat:** Generic Soup (`<A: Authenticator>`) propagating through `TokenManager`, `Session`, `ForceClient`, and all API Handlers (`RestHandler`, `BulkHandler`, etc), combined with a verbose type-state `ForceClientBuilder` (`NoAuth`, `HasAuth`, `AuthenticatedBuilder`).
**Cut:** Replaced the generic bound with dynamic dispatch (`Box<dyn Authenticator>`) inside `TokenManager`. Deleted the type-state builder logic and made `ForceClientBuilder` a simple, concrete struct requiring an authenticator in its `new()` constructor.
**Saved:** Removed thousands of repetitive `<A: Authenticator>` generics and type markers throughout the codebase, making the library significantly easier to use and reason about for end-users. The "zero-cost abstraction" of avoiding `dyn Trait` was an over-engineered optimization for an authentication strategy that only fires an HTTP request once an hour. Simplicity wins over theoretical nano-second performance gains.
