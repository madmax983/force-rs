**Cleaned up module visibilities in force and force-sync**
**Tangle:** Several modules were exposed publicly without needing to be, which leaked implementation details, violated YAGNI, and broke the "Encapsulate" rule of using `pub(crate)` by default for internal structure.
**Blueprint:** Refactored various `pub mod` to `pub(crate) mod` in `crates/force/src/api/mod.rs` and `crates/force/src/lib.rs` while ensuring public exports from `force/src/schema/mod.rs` remain properly exported, leaving internal implementation modules hidden. Restored explicit paths to internal tests. Reduced public API surface.

**Cleaned up module visibilities in force and force-sync**
**Tangle:** Several modules were exposed publicly without needing to be, which leaked implementation details, violated YAGNI, and broke the "Encapsulate" rule of using `pub(crate)` by default for internal structure.
**Blueprint:** Refactored various `pub mod` to `pub(crate) mod` in `crates/force/src/api/mod.rs` and `crates/force/src/lib.rs` while ensuring public exports from `force/src/schema/mod.rs` remain properly exported, leaving internal implementation modules hidden. Restored explicit paths to internal tests. Reduced public API surface.

**Extracted `test_utils` to break circular dependency**
**Tangle:** The `test_support` module inside the `force` crate created circular dependencies where core modules (like `auth` and `types`) depended on `test_support` for mocking, but `test_support` depended back on them to instantiate those same mocks. This violated the unidirectional dependency graph and created cyclic module coupling.
**Blueprint:** Created a new internal `test_utils` module containing `must`, `mock_auth`, and `mock_describe`. Moved the definitions of test macros and builders into this separate crate-level space so that `test_support` only acts as a facade re-exporting these tools, breaking the cycle and keeping the internal module boundaries acyclic.

**Refactored test_support to test_utils to break circular dependencies**
**Tangle:** The `test_support.rs` module created a circular dependency tangle (`types -> auth -> test_support -> types`) by functioning as a facade that test-only modules imported heavily, breaking strict DAG encapsulation.
**Blueprint:** Removed `test_support.rs` entirely. Updated all internal module tests to import mock builders and auth directly from the newly encapsulated `test_utils` internal module (`test_utils::mock_auth`, `test_utils::mock_describe`, and `test_utils::must`), enforcing unidirectional graph dependencies.
**[Unify error handling]
**Tangle:** Inconsistent error handling across modules where Result was used with explicit types.
**Blueprint:** Standardized error types across all modules to enforce domain boundaries.
**[Encapsulate test_utils submodules]
**Tangle:** The internal `test_utils` module in the `force` crate defined its submodules (`mock_auth`, `mock_describe`, `must`) as fully `pub`, which broke the "Encapsulate" rule by unnecessarily exposing internal testing tools beyond their intended crate-internal scope.
**Blueprint:** Modified the submodule declarations in `crates/force/src/test_utils/mod.rs` to use `pub(crate) mod` instead of `pub mod`, properly restricting their visibility to the crate boundary and preventing accidental external dependencies.
**[Acknowledge Visibility Bounds]
**Tangle:** Attempted to "fix" internal test_utils visibility by changing `pub mod` to `pub(crate) mod`, despite the parent `test_utils` module already being strictly `pub(crate)`.
**Blueprint:** Acknowledged that in Rust, a child module's maximum visibility is bound by its parent. Using `pub(crate)` inside an already restricted `pub(crate)` module is not only redundant but triggers clippy lints. No architecture change was necessary as the system was already sound.
