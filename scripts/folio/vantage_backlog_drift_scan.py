#!/usr/bin/env python3
"""Folio vantage-backlog drift scan: catch vantage specs for shipped surfaces.

`docs/vantage/README.md` frames itself as a "design backlog... for incubating
force-rs capabilities" that was "moved out of the repo root so release-facing
docs stay focused on shipped crates." That framing is a factual claim: every
entry in it is, by the directory's own definition, NOT YET shipped.

`docs/guide/surfaces/README.md` is the corpus's already-validated ground
truth for what HAS shipped (scripts/folio/surfaces_drift_scan.py keeps it
honest against the client's real accessors). A vantage spec whose subject is
also a real row in that table is a contradiction: the corpus is telling the
reader in one place "this is unbuilt, here's the spec" and in another
"this shipped, here's how to use it." Readers who land on the backlog first
(it sorts alphabetically before the guide in most file browsers) walk away
thinking a GA, ADR-documented surface doesn't exist yet.

MAPPING is a small, explicit, checked-in correspondence between a vantage
spec's filename and the feature flag it specs out. It is intentionally
hand-maintained (like CFG_ACCESSOR_RE's structural assumptions in the sibling
script) rather than guessed from substring overlap, because a false-positive
match here would delete or flag the wrong page.

Usage:
    python3 scripts/folio/vantage_backlog_drift_scan.py
Exit status is nonzero iff any drift is found, so this can gate CI.
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
VANTAGE_DIR = REPO_ROOT / "docs" / "vantage"
SURFACES_TABLE = REPO_ROOT / "docs" / "guide" / "surfaces" / "README.md"

# vantage spec filename -> feature flag it is a pre-implementation spec for.
# Only surfaces with an unambiguous 1:1 vantage-doc-to-feature-flag mapping
# are listed; anything not in this dict is out of scope for this check.
MAPPING: dict[str, str] = {
    "apex-rest-api.md": "apex_rest",
    "consent-portability-api.md": "consent",
    "data-cloud-api.md": "data_cloud",
    "soap-api.md": "soap",
}


def shipped_feature_flags() -> set[str]:
    """Feature flags that already have a row in the validated surfaces table."""
    text = SURFACES_TABLE.read_text()
    return set(re.findall(r"`([a-z_]+)`", text.split("## Surfaces", 1)[-1]))


def find_drift() -> list[dict]:
    shipped = shipped_feature_flags()
    findings = []
    for filename, flag in MAPPING.items():
        path = VANTAGE_DIR / filename
        if not path.exists():
            continue
        if flag in shipped:
            findings.append(
                {
                    "surface": filename,
                    "detail": (
                        f"docs/vantage/{filename} specs out feature `{flag}` as "
                        f"an incubating capability, but `{flag}` already has a "
                        f"shipped row in {SURFACES_TABLE.relative_to(REPO_ROOT)}"
                    ),
                }
            )
    return findings


def run() -> int:
    findings = find_drift()
    print("# Folio vantage-backlog drift scan")
    print(f"# backlog dir:  {VANTAGE_DIR.relative_to(REPO_ROOT)}")
    print(f"# ground truth: {SURFACES_TABLE.relative_to(REPO_ROOT)}")
    print(f"# mapped specs checked: {len(MAPPING)}")
    print(f"# drift found: {len(findings)}")
    for f in findings:
        print(f"  - [STALE_BACKLOG] {f['surface']}: {f['detail']}")
    return 1 if findings else 0


if __name__ == "__main__":
    sys.exit(run())
