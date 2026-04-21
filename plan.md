1.  **Refactor module visibility in `crates/force-sync`:**
    *   Change `pub mod` to `pub(crate) mod` in `crates/force-sync/src/store/pg/mod.rs` using `sed`.
    *   Change `pub mod` to `pub(crate) mod` in `crates/force-sync/src/store/mod.rs` using `sed`.
    *   Change `pub mod` to `pub(crate) mod` in `crates/force-sync/src/capture/mod.rs` using `sed`.
    *   Change `pub mod` to `pub(crate) mod` in `crates/force-sync/src/apply/mod.rs` using `sed`.

2.  **Verify Module Modifications:**
    *   Run `grep -rn "pub mod" crates/force-sync/src/` to verify that no `pub mod` declarations remain in the source directory (except those properly encapsulated).
    *   Run `cargo clippy --all-targets --all-features -- -D warnings`.
    *   Run `cargo test --all-features`.
    *   Run `cargo fmt --all`.

3.  Complete pre-commit steps to ensure proper testing, verification, review, and reflection are done.

4.  **Submit the architectural change:**
    *   Use `attempt_completion` to submit the branch with PR title "🗺️ Atlas: [Enforce Module Boundaries in force-sync]" and description including Tangle, Blueprint, Stability, and Verification sections.
