1. **Explore the missed mutants and the files:**
   I ran `cargo mutants` on `crates/force/src/client/mod.rs` and it found 9 missed mutants out of 22 tested.

   These mutants are mostly related to constructor initializations in `client/mod.rs` delegating to `BulkHandler::new()`, `CompositeHandler::new()`, etc.:
   - `crates/force/src/client/mod.rs:33:5: replace builder -> ForceClientBuilder<NoAuth> with ForceClientBuilder::from(Default::default())`
   - `crates/force/src/client/mod.rs:91:9: replace ForceClient<A>::bulk -> crate::api::bulk::BulkHandler<A> with BulkHandler::new(...)`
   - `crates/force/src/client/mod.rs:107:9: replace ForceClient<A>::composite -> crate::api::composite::CompositeHandler<A> with CompositeHandler::new(...)`

2. **Add tests to `crates/force/src/client/mod.rs` to cover these mutants:**
   I will write a couple of focused tests inside `crates/force/src/client/mod.rs` to verify that `bulk()` and `composite()` return handlers with the identical `inner` reference. This effectively kills all mutants that try to construct a new dummy handler instead of forwarding the actual `inner` session.
   I've already added `test_bulk_handler_points_to_same_inner` and `test_composite_handler_points_to_same_inner`.

   Wait, I already fixed these partially in my previous step, but let me check if `cargo mutants` still misses them.

3. **Re-run `cargo mutants` on `client/mod.rs` to see if there are still missed mutants.**

4. **Address remaining missed mutants** by adding precise assertions to tests. I'll make sure the `builder()` test checks that the builder state hasn't been substituted with a completely default config (e.g. by instantiating it and verifying specific fields that differ from default, or just verifying that `builder()` produces something that resolves correctly).

5. **Journal the findings** in `.jules/elenchus.md` as required by the persona.
