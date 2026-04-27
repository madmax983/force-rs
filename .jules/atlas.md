**Cleaned up module visibilities in force and force-sync**
**Tangle:** Several modules were exposed publicly without needing to be, which leaked implementation details, violated YAGNI, and broke the "Encapsulate" rule of using `pub(crate)` by default for internal structure.
**Blueprint:** Refactored various `pub mod` to `pub(crate) mod` in `crates/force/src/api/mod.rs` and `crates/force/src/lib.rs` while ensuring public exports from `force/src/schema/mod.rs` remain properly exported, leaving internal implementation modules hidden. Restored explicit paths to internal tests. Reduced public API surface.

**Cleaned up module visibilities in force and force-sync**
**Tangle:** Several modules were exposed publicly without needing to be, which leaked implementation details, violated YAGNI, and broke the "Encapsulate" rule of using `pub(crate)` by default for internal structure.
**Blueprint:** Refactored various `pub mod` to `pub(crate) mod` in `crates/force/src/api/mod.rs` and `crates/force/src/lib.rs` while ensuring public exports from `force/src/schema/mod.rs` remain properly exported, leaving internal implementation modules hidden. Restored explicit paths to internal tests. Reduced public API surface.
