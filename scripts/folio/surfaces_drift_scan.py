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

# Every handler accessor (macro-generated or hand-written) is immediately
# preceded by `#[cfg(feature = "<flag>")]`, optionally followed by other
# attributes (e.g. `#[must_use]`) before the `pub fn <name>`. Scanning for
# this pairing ties each accessor to the exact feature flag that gates it,
# instead of guessing from substring overlap between flag and fn names.
CFG_ACCESSOR_RE = re.compile(
    r'#\[cfg\(feature = "(?P<flag>[a-z_]+)"\)\]'
    r"(?:\s*#\[[^\]]*\])*\s*"
    r"pub fn (?P<name>[a-z_]+)"
)


def shipped_surfaces() -> dict[str, str]:
    """Map feature flag -> accessor name for every `client.<name>()` handler."""
    text = CLIENT_MOD.read_text()
    return {m.group("flag"): m.group("name") for m in CFG_ACCESSOR_RE.finditer(text)}


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
            # A cell can name more than one flag, e.g. an umbrella surface
            # like "`agentforce` (= `models` + `agent_api`)" -- capture all
            # of them so a compound row still covers each underlying flag.
            all_flags = re.findall(r"`([a-z_]+)`", feature)
            rows.append(
                {
                    "surface": surface,
                    "accessor": accessor,
                    "feature": feature_flag,
                    "all_flags": all_flags or [feature_flag],
                    "description": description,
                }
            )
    return rows


def find_drift() -> list[dict]:
    surfaces = shipped_surfaces()  # feature flag -> accessor name
    rows = parse_table_rows(SURFACES_TABLE.read_text())
    table_flags = {flag for row in rows for flag in row["all_flags"]}

    findings = []
    for row in rows:
        flag = row["feature"]
        code_accessor = surfaces.get(flag)
        if code_accessor is None:
            continue  # not a shipped client.<fn>() surface (e.g. umbrella/meta features); nothing to cross-check

        stale_status = STALE_STATUS_RE.search(row["description"])
        claims_no_accessor = row["accessor"].strip() == "—"

        # A real accessor exists for this flag, so the row is wrong if it
        # still reads as unshipped (stale wording) OR still shows no
        # accessor at all -- either shape leaves a shipped surface looking
        # unavailable, whether or not the two contradict each other.
        if stale_status or claims_no_accessor:
            claim = repr(stale_status.group(0)) if stale_status else "accessor '—'"
            findings.append(
                {
                    "type": "STALE_PLANNED",
                    "surface": row["surface"],
                    "detail": (
                        f"table row says {claim} for feature `{flag}`, but "
                        f"`client.{code_accessor}()` exists in {CLIENT_MOD.relative_to(REPO_ROOT)}"
                    ),
                }
            )

    for flag, accessor in surfaces.items():
        if flag not in table_flags:
            findings.append(
                {
                    "type": "MISSING_ROW",
                    "surface": accessor,
                    "detail": (
                        f"`client.{accessor}()` is gated by feature `{flag}` in "
                        f"{CLIENT_MOD.relative_to(REPO_ROOT)} but has no row in "
                        f"{SURFACES_TABLE.relative_to(REPO_ROOT)}"
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
