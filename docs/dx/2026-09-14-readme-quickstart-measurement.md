# Onramp measurement: README "first run" journey (2026-09-14)

Follow-up to `docs/dx/2026-09-14-readme-quickstart-baseline.md`. Same
harness (`cargo test -p force --test dx_readme_quickstart --all-features --
--test-threads=1`), rerun after the fix in this commit.

## Before / after

| Check | Before | After |
| --- | --- | --- |
| `readme_quickstart_compiles_from_a_clean_room` | FAIL — `E0599: no method named 'query'` (missing `use force::api::RestOperation;`) | **PASS** |
| `readme_version_pins_admit_the_current_release` | FAIL — 7 pins read `"0.1"` vs. current `0.4.0` | **PASS** |
| Broken-snippet count (README front-door Quick Start) | 1 | 0 |
| Snippet CI for this journey | none | permanent — runs on every `cargo test --workspace --all-features --lib --tests` (already in `fast-gates` and `full-matrix` CI jobs, no new workflow needed) |

```
$ cargo test -p force --test dx_readme_quickstart --all-features -- --test-threads=1
running 2 tests
test readme_quickstart_compiles_from_a_clean_room ... ok
test readme_version_pins_admit_the_current_release ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 23.32s
```

## Change applied

`README.md` only:

- Added `use force::api::RestOperation;` to the Quick Start code block (the
  missing import that broke `.query()`).
- Bumped all 7 stale `force = "0.1"` pins (Installation section + 4
  "Advanced Examples" comments + Preview Features section) to
  `force = "0.4"`, matching the current release
  (`[workspace.package] version` in `Cargo.toml`).

## Impact floor

Clears: *"The clean-room run fails today and passes after"* — both
independent hard failures on the documented front-door path now pass, from
the same harness, unmodified.

## Compatibility check

`git diff --stat -- crates/*/src` between the parent commit and this one is
empty: no production code changed, so there is no public API surface to
regress. `cargo fmt --all -- --check` and
`cargo clippy -p force --all-features --all-targets -- -D warnings` are
clean on the new test file.

## Known gap / follow-up (not fixed here, to keep this change to one layer)

- The README's "Advanced Examples" (GraphQL, Bulk) and "Feature Flags
  Summary" table are not covered by `dx_readme_quickstart.rs` and were not
  audited for drift in this pass — the harness intentionally covers only
  the canonical Quick Start block that's explicitly said to mirror
  `examples/readme_quick_start.rs`. The Feature Flags Summary table is also
  visibly stale (missing `data_cloud`, `apex_rest`, `cpq`, `consent`,
  `models`, `agent_api`, `account_engagement`, `analytics`, `soap`,
  `auth_code`, `username_password`, `files`, `agentforce`); worth a
  follow-up pass with its own before/after measurement rather than folding
  it into this fix.
- No public issue tracker / question-log volume was available to mine
  (personal project, no historical backlog) — Tier 1 deterministic evidence
  was the sole gate here, as the hard-gate rules allow.
