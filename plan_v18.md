Ah, I see what happened. The PR commit triggered the GitHub Actions CI, which runs `cargo clippy --workspace --all-features --all-targets -- -D warnings`.

Wait! The error in the CI is:
```
error[E0277]: the trait bound `session::Session<A>: std::default::Default` is not satisfied
  --> crates/force/src/client/mod.rs:91:44
   |
91 |         crate::api::bulk::BulkHandler::new(Default::default())
```
Wait a second, did I commit the file with `Default::default()` in `bulk()`?
Let me check the diff of my last commit! I must have accidentally left the manual mutant in `crates/force/src/client/mod.rs` when I recreated my tests!
