**Removed Circular Dependencies in `crates/force/src`**
**Tangle:** Circular `use` statements between `crate::error`, `crate::test_support`, `crate::auth`, and `crate::types` caused by incorrect absolute `crate::test_support` paths in test modules across the crate.
**Blueprint:** Updated test modules to use relative paths (`super::`) across `crates/force/src` to avoid circular references during compilation, and properly encapsulated modules in `crates/force-sync/src` using `pub(crate)` without redundant visibility nesting.
