**Remove unnecessary clones from JwtClaims**
**Learning:** `jsonwebtoken::encode` takes claims by reference, meaning there's no need to serialize owned `String` fields. Borrowing `&'a str` fields on local, single-use structs avoids unnecessary allocations during high-frequency tasks like JWT generation.
**Action:** Always check if a struct passed by reference to a third-party crate function actually needs owned data. If it's a short-lived DTO, use lifetimes and `&str` instead of `.clone()`.

**[Optimizing Optional Telemetry]**
**Learning:** Constructing structs with `String` fields for telemetry events (e.g. `RequestCompletion`) allocates memory even if the event is never consumed (no hooks registered).
**Action:** Use `Option<String>` in the context struct and only populate it if telemetry hooks are active. Pass a capture flag to the context constructor.

**[Zero-Cost JSON Deserialization]**
**Learning:** `serde_json::from_value` requires taking ownership of `serde_json::Value`, forcing an expensive `.clone()` when the source is borrowed. However, `&Value` implements `serde::Deserializer`, so `T::deserialize(value)` can be used to deserialize directly from a reference without cloning the tree.
**Action:** Always prefer `T::deserialize(value)` over `serde_json::from_value(value.clone())` when reading fields from a borrowed JSON value to avoid heap allocations.

**[Pre-allocating Vecs for Graph Requests]**
**Learning:** `GraphBuilder::new()` and `Graph::new()` in `api/composite/graph.rs` construct arrays using `Vec::new()`, resulting in multiple heap allocations as elements are added, especially given the Salesforce limit is often up to 500 subrequests but typically batches are around 15.
**Action:** Use `Vec::with_capacity(15)` to avoid initial allocations and keep typical small batches entirely allocation-free during accumulation.
**Avoid `.clone()` in `Arc::new()`**
**Learning:** Initializing an `Arc` by cloning the source variable (`Arc::new(val.clone())`) instead of moving it (`Arc::new(val)`) causes a completely unnecessary deep copy and heap allocation.
**Action:** When transferring ownership of a newly created object to an `Arc` where the original variable is no longer needed, pass ownership directly without calling `.clone()`.
