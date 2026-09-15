# Onramp baseline: README "first run" journey (2026-09-14)

## Journey

**First run**: zero -> hello world for a brand-new `force` user. Weight: this
*is* the front door — `README.md` is the content crates.io and GitHub render
as the project's landing page (`readme = "../../README.md"` in
`crates/force/Cargo.toml`), and it is the only doc a developer sees before
deciding whether the crate works at all. No public issue tracker volume is
mined here (this is a personal project with no historical issue backlog to
bucket — Tier 2 evidence is not available and that gap is recorded rather
than papered over); the Tier-1 deterministic clean-room run below is the
primary and sufficient gate per the hard-gate rules.

Reproduce (the exact harness, wired permanently into `cargo test`, see
"Change" below):

```
cargo test -p force --test dx_readme_quickstart --all-features -- --test-threads=1
```

## Evidence (Tier 1, deterministic, this commit)

Two independent clean-room reproductions before any fix, both starting from
a throwaway scratch cargo project, extracting the *actual* `README.md`
Installation + Quick Start content programmatically (never a hand-typed
copy), both hard-failing to compile. Logs committed alongside this file:
`docs/dx/logs/clean-room-path-20260914T230904Z.log` (dependency pointed at
this checkout, i.e. what we're about to ship) and
`docs/dx/logs/clean-room-published-20260914T230933Z.log` (dependency left as
the verbatim registry pin README.md shows today, resolved against the real
crates.io registry — reproducing exactly what a developer who pastes the
docs verbatim gets right now).

| Mode | Dependency source | Result | Steps | Wall time | Failure |
| --- | --- | --- | --- | --- | --- |
| path (this checkout) | `crates/force` at `0.4.0` | **FAIL** | 11 | 23s | `E0599: no method named 'query' found for struct 'RestHandler<A>'` |
| published (verbatim registry pin) | crates.io `force = "0.1"` | **FAIL** | 10 | 22s | `E0599: no function or associated item named 'new_my_domain' found` |

Broken-snippet count: **1 of 1** primary Quick Start snippets tested (the
canonical block explicitly said to mirror `examples/readme_quick_start.rs`).
No snippet CI existed before this commit — `cargo test --workspace --doc
--all-features` runs only rustdoc doctests inside `src/`, and no CI job
built `examples/readme_quick_start.rs` or any README fence — so this defect
had no mechanism to ever be caught. Building that mechanism is itself part
of this change (see below).

Static check, no network required: parsing `README.md` for every `force =
"X.Y"` / `force = { version = "X.Y", ... }` pin finds **7** occurrences, all
reading `"0.1"`, none of which admit the current release (`0.4.0`):

```
README.md:66, 72, 134, 176, 220, 329, 333: force = "0.1" does not match 0.4.0
```

## Hypothesis (mechanism)

Two independent, compounding failures on the same journey:

1. **Drifted copy, no sync mechanism.** The top-level `README.md` Quick
   Start block is a hand-copy of `crates/force/examples/readme_quick_start.rs`.
   ADR-019 extracted CRUD/Query/Describe onto a separate `RestOperation`
   trait, so the example was updated to add
   `use force::api::RestOperation;` — but the README copy was not. Nothing
   builds the README snippet, so the drift compiles silently in one place
   (the example, which *is* built) and breaks silently in the other (the
   README, which is not). A developer who does exactly what the crate's own
   crates.io/GitHub page says — `cargo add force`, paste the Quick Start,
   `cargo build` — gets `E0599` with no clue that a trait import is the
   missing piece, on the very first snippet they try.
2. **Stale version pin, no drift alarm.** `README.md`'s Installation section
   has pinned `force = "0.1"` since (at least) the 0.1 release and was never
   bumped across the 0.2, 0.3, and 0.4 releases. Because 0.1.0 is real,
   published, and not yanked, `cargo build` never errors on the pin itself —
   it silently resolves three releases behind, so a developer who is more
   careful and reads `Cargo.toml` literally still lands on dead API surface
   (`ClientCredentials::new_my_domain` does not exist in 0.1.0) with no
   signal that the version number itself is the defect.

## Impact floor

Clears immediately: *"The clean-room run fails today"* — hard failure on the
documented path in both modes above, on the crate's actual front door.

## Change (this PR)

Layer: **docs** (no API, no default, no error-message change — the code is
already correct; the copy of it in the README is not) plus a **permanent
regression test** so this class of drift cannot recur silently again:

- `crates/force/tests/dx_readme_quickstart.rs` — two `#[test]`s, run by the
  existing `cargo test --workspace --all-features --lib --tests` step
  already in `fast-gates` (and again in `full-matrix`), no new CI job
  needed:
  - `readme_quickstart_compiles_from_a_clean_room`: extracts the literal
    Installation + Quick Start content from `README.md`, builds it in a
    throwaway project pointed at this checkout, fails the build if it
    doesn't compile.
  - `readme_version_pins_admit_the_current_release`: parses every `force =
    "X.Y"` pin out of `README.md` and fails if any doesn't admit
    `CARGO_PKG_VERSION`.
- `README.md`: add the missing `use force::api::RestOperation;` import to
  the Quick Start block; bump all 7 stale `force = "0.1"` pins to
  `force = "0.4"`.

Compatibility: zero `crates/*/src` changes — public API surface is untouched
by construction (verified: `git diff --stat -- crates/*/src` is empty).
