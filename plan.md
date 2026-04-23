1. **Refactor `write_dbml` to eliminate `.join(", ")` heap allocations**
   - In `crates/force/src/schema/dbml_generator.rs`, the `write_dbml` function allocates a `Vec<String>` named `settings`, pushes formatted strings to it, and then calls `settings.join(", ")` to generate a final string for writing.
   - I will eliminate the intermediate vector and the `.join()` allocation by appending characters directly to the `out` buffer or using `format!` inline within a loop.
   - This prevents memory allocations per field when generating DBML outputs.

2. **Run tests to verify the optimization**
   - `cargo test` in the `force` crate to ensure DBML output generation is intact and all tests pass.

3. **Complete pre-commit steps to ensure proper testing, verification, review, and reflection are done.**
   - Run `cargo fmt --all`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-features`

4. **Submit PR**
   - Create a PR titled `⚡ Bolt: [performance improvement] Eliminate heap allocations in DBML generation`
   - Include `💡 What`, `🎯 Why`, `📊 Impact`, and `🔬 Measurement` in the description.
