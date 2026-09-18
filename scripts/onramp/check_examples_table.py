#!/usr/bin/env python3
"""Onramp DX harness: verify README.md's "More Examples" table against
reality.

The table (`## More Examples`) is the front door for the *second* journey a
developer takes with this crate -- past hello-world, into "I need the
GraphQL / Bulk / Tooling example for my use case." Each row makes two claims:

    | [`bulk_insert.rs`](crates/force/examples/bulk_insert.rs) | `bulk` | ... |

  1. The linked example file exists.
  2. `--features <that one feature>` is sufficient to compile it -- which is
     exactly the command the row teaches the reader to copy: `cargo run
     --example bulk_insert --features bulk`.

Nothing else in CI checks claim #2 in isolation. `cargo clippy --workspace
--all-features --all-targets` (fast-gates in ci.yml) compiles every example,
but with every feature unioned together, so a row's single-feature claim
could be wrong -- missing a dependency the row doesn't mention, or naming a
feature the example doesn't even need -- and every existing CI job would
still stay green. This is the same class of gap that let the README Quick
Start snippet drift (see scripts/onramp/compile_readme_snippets.sh and
scripts/onramp/check_version_pins.sh): a documented command that nothing
ever actually runs.

Usage:
    python3 scripts/onramp/check_examples_table.py
"""
from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
README = REPO_ROOT / "README.md"
EXAMPLES_DIR = REPO_ROOT / "crates" / "force" / "examples"

# One row: | [`name.rs`](crates/force/examples/name.rs) | `feature` | ... |
ROW_RE = re.compile(
    r"^\|\s*\[`(?P<name>[a-zA-Z0-9_]+)\.rs`\]\((?P<path>[^)]+)\)\s*"
    r"\|\s*`(?P<feature>[a-zA-Z0-9_]+)`\s*\|"
)

# A markdown table separator row, e.g. `|---------|---------|-------------|`.
SEPARATOR_RE = re.compile(r"^\|[\s\-:|]+\|$")


def section_after_heading(text: str, heading: str) -> str:
    idx = text.index(heading)
    rest = text[idx + len(heading):]
    next_heading = re.search(r"\n## ", rest)
    return rest[: next_heading.start()] if next_heading else rest


def parse_table(text: str) -> tuple[list[tuple[str, str, str]], list[str]]:
    """Returns (rows, unparsed_lines). A `|`-prefixed line in the table that
    is neither the header nor the separator nor a row `ROW_RE` matches is a
    parse failure, not something to silently skip -- an unparseable row
    (renamed column, hyphenated example name, mangled markdown) would
    otherwise vanish from the check entirely while the harness still exits
    0, defeating its own purpose exactly when the table drifts."""
    section = section_after_heading(text, "## More Examples")
    rows = []
    unparsed = []
    seen_header = False
    for raw_line in section.splitlines():
        line = raw_line.strip()
        if not line.startswith("|"):
            continue
        if SEPARATOR_RE.match(line):
            continue
        if not seen_header:
            # First non-separator `|` line is the header row; skip it.
            seen_header = True
            continue
        m = ROW_RE.match(line)
        if m:
            rows.append((m.group("name"), m.group("path"), m.group("feature")))
        else:
            unparsed.append(line)
    return rows, unparsed


def main() -> int:
    text = README.read_text()
    rows, unparsed = parse_table(text)
    if not rows:
        print("ERROR: found no example rows under '## More Examples' in README.md")
        print("(the table format may have changed -- update ROW_RE in this script)")
        return 1

    print(f"Checking {len(rows)} example(s) from README.md's 'More Examples' table...")
    failures: list[str] = []

    if unparsed:
        for line in unparsed:
            failures.append(
                f"could not parse table row (does not match ROW_RE -- update the "
                f"regex or fix the row): {line}"
            )

    for name, path, feature in rows:
        example_file = REPO_ROOT / path
        if not example_file.is_file():
            failures.append(f"{name}: linked path '{path}' does not exist")
            continue
        if example_file != EXAMPLES_DIR / f"{name}.rs":
            failures.append(
                f"{name}: link path '{path}' does not point at "
                f"crates/force/examples/{name}.rs"
            )
            continue

        # Exactly the command the row teaches: `cargo run --example NAME
        # --features FEATURE`. `check` instead of `run` -- we're verifying it
        # compiles, not exercising the live Salesforce calls inside it.
        cmd = [
            "cargo", "check", "-p", "force",
            "--example", name,
            "--features", feature,
            "--quiet",
        ]
        result = subprocess.run(cmd, cwd=REPO_ROOT, capture_output=True, text=True)
        if result.returncode != 0:
            failures.append(
                f"{name}: `cargo check --example {name} --features {feature}` "
                f"failed (README claims `{feature}` is sufficient):\n"
                f"{result.stderr.strip()}"
            )
        else:
            print(f"  OK  {name} (--features {feature})")

    if failures:
        print(f"\n{len(failures)} of {len(rows)} example row(s) are stale:\n")
        for f in failures:
            print(f"  FAIL {f}\n")
        return 1

    print(f"\nAll {len(rows)} example rows compile exactly as documented.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
