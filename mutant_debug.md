Oh, I see what's happening. The mutants are `MISSED` because `cargo mutants` only runs `cargo test` on the entire workspace.
Wait, `cargo test` on the workspace DOES execute tests in `crates/force/src/client/mod.rs`. So why are the tests passing if the mutant is injected?

Let's do this: I will manually apply the mutant and run `cargo test --workspace` and SEE if my test fails.
