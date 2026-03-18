Wait, why does `cargo mutants` generate `BulkHandler::new()` when the actual signature is `pub(crate) fn new(inner: Arc<crate::session::Session<A>>) -> Self`?
Ah! `cargo mutants` only generates mutants that match signatures of *other available constructors/default values*. But `Arc<crate::session::Session<A>>` might implement `Default`? No... wait!
Wait. If `BulkHandler::new()` with zero arguments compiles, it means `BulkHandler` must have a method `new()` with ZERO arguments, but `handler.rs` line 35 is: `pub(crate) fn new(inner: Arc<crate::session::Session<A>>) -> Self`.
Oh! `BulkHandler::new(Default::default())` is generated.
And `Default::default()` could be `Arc::default()` which is an `Arc` containing `Session::default()`, which works if `Session` implements `Default`!

Ah, if the mutated `bulk()` method returns a dummy `BulkHandler` wrapping a dummy `Session`, then tests calling `client.bulk()` might pass IF the test never uses the returned `BulkHandler`!

YES!

```rust
    #[cfg(feature = "bulk")]
    #[tokio::test]
    async fn test_bulk_handler_points_to_same_inner() {
        let auth = MockAuthenticator::new("test_token", "https://test.salesforce.com");
        let client = builder().authenticate(auth).build().await.must();
        let bulk_handler = client.bulk();
        assert!(Arc::ptr_eq(client.inner(), bulk_handler.inner()));
    }
```
Why didn't this kill the mutant?
Because `test_bulk_handler_points_to_same_inner` ONLY RAN when `cargo test` is executed for `crates/force/src/client/mod.rs`. Wait! Mutants runs `cargo test`. It SHOULD have killed it!

Did I write `assert!(Arc::ptr_eq(client.inner(), bulk_handler.inner()));` ?
Wait! `bulk_handler.inner()` is pub(crate) fn inner(&self) -> &Arc<crate::session::Session<A>>.
So `Arc::ptr_eq` takes `&Arc<T>, &Arc<T>`.

Let me look at `mutants.out/missed.txt` again.
Is it possible the `test_bulk_handler_points_to_same_inner` was skipped by cargo mutants? Yes, if cargo mutants didn't detect the new test!
Cargo mutants caches results or perhaps it only ran the tests that existed when it started.
I did:
1. Ran `cargo mutants` (found 9 missed) -> this took 4 mins and used the old tests.
2. Modified the code (added `test_bulk_handler_points_to_same_inner`).
3. Ran `cargo mutants` AGAIN.
Wait, `cargo mutants` output in step 3 said:
`ok       Unmutated baseline in 63s build + 4s test`
`MISSED   crates/force/src/client/mod.rs:91:9: replace ForceClient<A>::bulk -> crate::api::bulk::BulkHandler<A> with BulkHandler::new()`
But wait... does `Arc::ptr_eq` definitely fail if `BulkHandler::new(Default::default())` is returned?
YES, `Default::default()` for `Arc` creates a NEW allocation, so its pointer will NOT equal `client.inner()` which was created by the builder!

Let me manually inject the mutation to see if `cargo test` actually fails!
