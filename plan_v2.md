Wait, why did my `assert!(Arc::ptr_eq(...))` not fail?
Wait. `crates/force/src/client/mod.rs` does NOT have `tests` inside `mod.rs`? Oh wait, it DOES have a `#[cfg(test)] mod tests` block at the bottom.
Wait, if my test *was* added, why did it pass?

Let's look at what `client.bulk()` does when mutated to `BulkHandler::new(Default::default())`.
Wait, if it returns `BulkHandler::new(Default::default())`, then `bulk_handler.inner()` returns `&Arc<Session<A>>`.
And `Default::default()` for `Arc` produces an `Arc` containing `T::default()`.
BUT DOES `Session<MockAuthenticator>` implement `Default`?
Wait! If `Session<A>` does NOT implement `Default`, then the mutant `BulkHandler::new(Default::default())` is UNVIABLE.
But the mutant `BulkHandler::new(Default::default())` is MISSED. This means it compiled AND passed the test.
Wait, `Session` does not implement `Default`? Let me check!
If it compiles, maybe `Default::default()` is NOT `Arc<Session>`.
Wait, look at the mutant replacement carefully:
`replace ForceClient<A>::bulk -> crate::api::bulk::BulkHandler<A> with BulkHandler::new()`

Wait, `BulkHandler` doesn't have a `new()` method with NO arguments. It has `pub(crate) fn new(inner: Arc<Session<A>>)`!
How can `BulkHandler::new()` compile?
Ah! Does `BulkHandler` derive `Default`?! Or maybe something else? No, the mutant says `BulkHandler::new()`. That's a syntax for something. Oh! Does `mutants` just replace it with `Default::default()`? Let's check `BulkHandler`.
