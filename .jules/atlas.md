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
**[Broke Circular Dependency Between Client and API Modules]
**Tangle:** [The `client` module depended on `api` to instantiate handlers (`RestHandler`, `CompositeHandler`), while the `api` module depended on `client` because the `api::composite::query_batch::QueryBatch` and `api::composite::soql_mass_op::SoqlMassOp` types took `ForceClient` directly as a dependency to execute mixed REST and Composite operations.]
**Blueprint:** [Refactored `QueryBatch` and `SoqlMassOp` to take `Arc<Session<A>>` instead of `&ForceClient<A>`. Moved their construction into `CompositeHandler` (e.g., `client.composite().soql_mass_op(query)`). Inside these structs, `RestHandler::new(self.session.clone())` and `CompositeHandler::new(self.session.clone())` are now constructed dynamically directly from the session, completely decoupling `api` from `client`.]
