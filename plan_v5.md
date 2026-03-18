Ah! `git reset --hard` completely erased my manual modifications of `crates/force/src/client/mod.rs` where I added the tests. No wonder `cargo mutants` missed it! The test didn't exist in the codebase!

Let's do this again, correctly.

1. Add tests `test_bulk_handler_points_to_same_inner` and `test_composite_handler_points_to_same_inner` to `crates/force/src/client/mod.rs` using regex string replacements properly.
2. Add a `test_builder_creates_noauth_state` which is not empty, perhaps instantiating the builder and modifying it.
3. Run `cargo mutants -v --file crates/force/src/client/mod.rs` to confirm they are killed.
4. Update the journal `.jules/elenchus.md`.
