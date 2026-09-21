#!/usr/bin/env python3
"""Onramp DX harness: diff docs/guide/01-getting-started.md's "Feature-flag
matrix" -- the table a newcomer reads immediately after Quick Start to decide
what to put in `Cargo.toml` -- against `crates/force/Cargo.toml`'s `[features]`
table, the ground truth.

Nothing else in this repository catches this drift:
  * `scripts/folio/surfaces_drift_scan.py` checks a *different* table
    (docs/guide/surfaces/README.md) against the shipped client accessors.
  * `scripts/onramp/onramp_snippets.py check-version-pins` checks `force = "X"`
    version pins, not feature names.
  * Nothing diffs the getting-started matrix's own "From the actual
    definitions" `full`/`all` code block against the real meta-features.

So a feature can ship (new `[features]` entry, added to `full`/`all`) and the
getting-started page -- which claims to enumerate "every gate defined in
crates/force/Cargo.toml" -- silently stops being true. A newcomer deciding
"does this crate support X" from that table gets a wrong answer with no
signal that anything is stale.

Exit code is non-zero if the table is missing a shipped feature, or if the
`full`/`all` code block doesn't match the real meta-feature definitions.

Usage:
    python3 scripts/onramp/check_feature_matrix.py
"""
from __future__ import annotations

import re
import sys
import tomllib
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
CARGO_TOML = REPO_ROOT / "crates" / "force" / "Cargo.toml"
GETTING_STARTED = REPO_ROOT / "docs" / "guide" / "01-getting-started.md"

# Features that are deliberately absent from the newcomer-facing matrix: they
# are internal-only and Cargo.toml says so explicitly. Adding a feature here
# without that same "internal-only" marker in Cargo.toml is itself a bug in
# this allowlist, so it's checked below rather than trusted blindly.
INTENTIONALLY_UNDOCUMENTED = {"bench-internals"}

TABLE_ROW_RE = re.compile(r"^\|\s*`([a-zA-Z0-9_-]+)`\s*\|", re.MULTILINE)
DEFINITIONS_BLOCK_RE = re.compile(
    r"From the actual definitions:\s*\n```toml\n(.*?)```", re.DOTALL
)


def cargo_features() -> dict[str, list[str]]:
    data = tomllib.loads(CARGO_TOML.read_text())
    return data["features"]


def documented_feature_rows(text: str) -> set[str]:
    return set(TABLE_ROW_RE.findall(text))


def documented_definitions_block(text: str) -> dict[str, list[str]]:
    match = DEFINITIONS_BLOCK_RE.search(text)
    if not match:
        return {}
    return tomllib.loads(match.group(1))


def main() -> int:
    features = cargo_features()
    doc_text = GETTING_STARTED.read_text()
    documented_rows = documented_feature_rows(doc_text)
    documented_defs = documented_definitions_block(doc_text)

    problems: list[str] = []

    for name in INTENTIONALLY_UNDOCUMENTED:
        if name not in features:
            problems.append(
                f"'{name}' is allowlisted as intentionally-undocumented in "
                f"this script but no longer exists in {CARGO_TOML.relative_to(REPO_ROOT)} "
                "-- remove it from INTENTIONALLY_UNDOCUMENTED."
            )

    shipped_public_features = set(features) - INTENTIONALLY_UNDOCUMENTED
    missing_rows = sorted(shipped_public_features - documented_rows)
    for name in missing_rows:
        problems.append(
            f"feature `{name}` is defined in Cargo.toml but has no row in "
            f"{GETTING_STARTED.relative_to(REPO_ROOT)}'s Feature-flag matrix table."
        )

    stale_rows = sorted(documented_rows - set(features))
    for name in stale_rows:
        problems.append(
            f"the Feature-flag matrix table documents `{name}`, which no "
            "longer exists in Cargo.toml -- remove or rename the row."
        )

    if not documented_defs:
        problems.append(
            "could not find the \"From the actual definitions:\" ```toml "
            f"block in {GETTING_STARTED.relative_to(REPO_ROOT)} -- has it moved or been reworded?"
        )
    else:
        for meta in ("full", "all"):
            real = features.get(meta, [])
            documented = documented_defs.get(meta, [])
            if list(real) != list(documented):
                problems.append(
                    f"`{meta}` in the \"actual definitions\" block reads "
                    f"{documented!r} but Cargo.toml's real `{meta}` is "
                    f"{list(real)!r}."
                )

    if problems:
        print(
            f"Checked {len(features)} feature(s) in {CARGO_TOML.relative_to(REPO_ROOT)} "
            f"against {GETTING_STARTED.relative_to(REPO_ROOT)}'s Feature-flag matrix."
        )
        print(f"FAIL: {len(problems)} drift issue(s) found:\n")
        for p in problems:
            print(f"  - {p}")
        return 1

    print(
        f"Checked {len(features)} feature(s) in {CARGO_TOML.relative_to(REPO_ROOT)} "
        f"against {GETTING_STARTED.relative_to(REPO_ROOT)}'s Feature-flag matrix: no drift."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
