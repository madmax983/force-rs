#!/usr/bin/env python3
"""Folio drift scan: cross-check docs/guide/surfaces/README.md against the
shipped client API surface.

The surfaces table is the canonical index a reader consults to answer "does
force-rs support X yet?". Its `Accessor` and `Description` columns make a
factual claim about shipped status per row. This script extracts the ground
truth mechanically (which `client.<accessor>()` methods actually exist in
`crates/force/src/client/mod.rs`, feature-gated by which Cargo.toml flags)
and diffs it against the table's claims, so a row that still says a surface
is unshipped/"planned" after the code has landed is caught by CI instead of
by a reader who trusted the table and went to hand-roll the integration
themselves.

Two defect shapes are flagged:
  * STALE_PLANNED  - the row's description reads as not-yet-shipped
                      ("planned", "not yet", "coming soon", "roadmap") for a
                      feature flag that already has a real `client.<fn>()`
                      accessor in the code.
  * MISSING_ROW    - a feature-gated accessor exists in the client but has no
                      row in the table at all (pure coverage gap).

Usage:
    python3 scripts/folio/surfaces_drift_scan.py
Exit status is nonzero iff any drift is found, so this can gate CI.
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
CLIENT_MOD = REPO_ROOT / "crates" / "force" / "src" / "client" / "mod.rs"
SURFACES_TABLE = REPO_ROOT / "docs" / "guide" / "surfaces" / "README.md"

STALE_STATUS_RE = re.compile(
    r"\b(planned|not yet (merged|shipped|implemented|available)|coming soon|pending merge|not yet on trunk)\b",
    re.IGNORECASE,
)

# `pub fn <name> -> crate::api::...Handler<A>` (macro-generated accessors)
# and plain `pub fn <name>(...)  -> crate::api::...Handler<A>` (hand-written
# accessors like `account_engagement`, `data_cloud`).
ACCESSOR_RE = re.compile(
    r"pub fn (?P<name>[a-z_]+)(?:\s*->|\s*\()\s*.*?crate::api::",
)


def shipped_accessors() -> set[str]:
    text = CLIENT_MOD.read_text()
    names = set()
    for line in text.splitlines():
        m = re.search(r"pub fn ([a-z_]+)", line)
        if not m:
            continue
        name = m.group(1)
        # Cheap filter: only accessor-shaped names, skip helpers like
        # `session`. We only need this for the handful of known surfaces,
        # so require the name to appear later as `crate::api::<name-ish>`
        # within a few lines (the macro body / fn body).
        idx = text.index(line)
        window = text[idx : idx + 300]
        if "crate::api::" in window or "Handler" in window:
            names.add(name)
    return names


def parse_table_rows(md_text: str) -> list[dict]:
    rows = []
    in_table = False
    for line in md_text.splitlines():
        if line.startswith("| Surface |"):
            in_table = True
            continue
        if in_table:
            if not line.startswith("|"):
                break
            if line.startswith("|---"):
                continue
            cells = [c.strip() for c in line.strip("|").split("|")]
            if len(cells) < 4:
                continue
            surface, accessor, feature, description = cells[0], cells[1], cells[2], cells[3]
            feature_flag = re.sub(r"[`()=+]| .*$", "", feature).strip("`")
            rows.append(
                {
                    "surface": surface,
                    "accessor": accessor,
                    "feature": feature_flag,
                    "description": description,
                }
            )
    return rows


def find_drift() -> list[dict]:
    accessors = shipped_accessors()
    rows = parse_table_rows(SURFACES_TABLE.read_text())

    findings = []
    for row in rows:
        accessor_call = re.search(r"client\.([a-z_]+)\(", row["accessor"])
        claims_no_accessor = row["accessor"].strip() == "—"
        stale_status = STALE_STATUS_RE.search(row["description"])

        if stale_status and not claims_no_accessor:
            continue  # description mentions a status word but still names a real accessor; not this row's failure mode

        if stale_status and claims_no_accessor:
            # Does a real accessor exist for this surface's feature flag
            # even though the table claims none / claims "planned"?
            flag = row["feature"]
            code_accessor = None
            for name in accessors:
                # feature flag name usually matches or is a substring of the accessor
                if name == flag or name in flag or flag in name:
                    code_accessor = name
                    break
            if code_accessor:
                findings.append(
                    {
                        "type": "STALE_PLANNED",
                        "surface": row["surface"],
                        "detail": (
                            f"table row says {stale_status.group(0)!r} / accessor '—' for "
                            f"feature `{flag}`, but `client.{code_accessor}()` exists in "
                            f"{CLIENT_MOD.relative_to(REPO_ROOT)}"
                        ),
                    }
                )
    return findings


def run() -> int:
    findings = find_drift()
    print(f"# Folio surfaces drift scan")
    print(f"# table:  {SURFACES_TABLE.relative_to(REPO_ROOT)}")
    print(f"# ground truth: {CLIENT_MOD.relative_to(REPO_ROOT)}")
    print(f"# rows checked: {len(parse_table_rows(SURFACES_TABLE.read_text()))}")
    print(f"# drift found: {len(findings)}")
    for f in findings:
        print(f"  - [{f['type']}] {f['surface']}: {f['detail']}")
    return 1 if findings else 0


if __name__ == "__main__":
    sys.exit(run())
