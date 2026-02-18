## [Reduction]
**Bloat:** `PubSubHandler` (Zombie Code). A placeholder file for a feature that wasn't implemented, pulling in heavy dependencies like `tonic` and `prost` just for a "future" feature.
**Cut:** Deleted `crates/force/src/api/pub_sub.rs` and the `pub_sub` feature.
**Saved:** 3 heavy dependencies, 1 file, and confusion for future maintainers.

## [Reduction]
**Bloat:** `QueryIterator` (Redundant Abstraction). A synchronous iterator wrapper around `Vec<QueryResult>` that encourages loading all pages into memory before iterating. Standard iterators like `flat_map` are sufficient and more idiomatic.
**Cut:** Deleted `QueryIterator` struct and tests.
**Saved:** ~50 lines of code and a potential performance pitfall.
