Wait, why did `assert!(Arc::ptr_eq(client.inner(), bulk_handler.inner()));` not run and kill the mutant?
Let me check `mutants.out/outcomes.json` again or just check why tests didn't fail.
When I manually applied the mutant `crate::api::bulk::BulkHandler::new(Default::default())` and ran `cargo test`, DID it fail?

Wait, in my `manual_mutant.py` output, `cargo test` actually PASSED!
Why?
Ah. `client.inner()` returns `&Arc<Session<A>>`.
Wait. If `BulkHandler::new(Default::default())` was called, how did it compile?
Let's see: `Arc<Session<A>>` implements `Default` because `Session<A>` implements `Default`?
Where does `Session<A>` implement `Default`?
In `crates/force/src/session.rs` maybe?
