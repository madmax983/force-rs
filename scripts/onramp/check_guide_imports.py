#!/usr/bin/env python3
"""Onramp snippet CI: every `use force::...;` in a guide ```rust fence must resolve.

The other onramp harnesses compile whole fragments, but only for README.md,
01-getting-started, 02-choosing-an-auth-flow and 06-upgrading. The per-surface
pages (docs/guide/surfaces/*.md) and 03-operations.md were never compiled, and
their first line is almost always an import -- the line a newcomer copies first.
A wrong import path fails there with E0432/E0603 before anything else runs.

This harness extracts each `use force::...;` statement from the ```rust fences
of README.md and docs/guide/**/*.md, writes them one per line into a scratch
crate (path dependency on the workspace `force`, `--features all`), runs
`cargo check --keep-going`, and maps every error back to doc:line.

Usage: python3 scripts/onramp/check_guide_imports.py   (exit 1 on any failure)
"""
from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
OUT = REPO_ROOT / "target" / "onramp" / "guide_imports"
FENCE_RE = re.compile(r"```rust\n(.*?)```", re.DOTALL)
USE_RE = re.compile(r"^[ \t]*(use force::[^;]*;)", re.MULTILINE)


def docs() -> list[Path]:
    return [REPO_ROOT / "README.md", *sorted((REPO_ROOT / "docs" / "guide").rglob("*.md"))]


def main() -> int:
    stmts: list[tuple[str, int, str]] = []  # (doc, doc line, statement)
    for doc in docs():
        text = doc.read_text()
        for fence in FENCE_RE.finditer(text):
            base = text.count("\n", 0, fence.start(1)) + 1
            body = fence.group(1)
            for m in USE_RE.finditer(body):
                line = base + body.count("\n", 0, m.start(1))
                stmts.append((str(doc.relative_to(REPO_ROOT)), line, " ".join(m.group(1).split())))
    if not stmts:
        print("No `use force::` statements found", file=sys.stderr)
        return 1

    (OUT / "src").mkdir(parents=True, exist_ok=True)
    (OUT / "Cargo.toml").write_text(
        f'[package]\nname = "onramp-guide-imports"\nversion = "0.0.0"\npublish = false\n'
        f'edition = "2021"\n\n[workspace]\n\n[dependencies]\n'
        f'force = {{ path = "{REPO_ROOT}/crates/force", features = ["all", "username_password"] }}\n'
    )
    # Line N of lib.rs is statement N (line 1 = the allow attribute). Each
    # statement gets its own module so repeated imports don't collide.
    lines = ["#![allow(unused_imports)]"] + [f"mod m{i} {{ {s} }}" for i, (_, _, s) in enumerate(stmts)]
    (OUT / "src" / "lib.rs").write_text("\n".join(lines) + "\n")

    proc = subprocess.run(
        ["cargo", "check", "--keep-going", "--color", "never", "--message-format", "short"],
        cwd=OUT, capture_output=True, text=True,
    )
    bad: dict[int, str] = {}
    for m in re.finditer(r"src/lib\.rs:(\d+):\d+: error(?:\[\w+\])?: (.*)", proc.stderr):
        idx = int(m.group(1)) - 2
        if 0 <= idx < len(stmts):
            bad.setdefault(idx, m.group(2))
    if proc.returncode != 0 and not bad:
        print(proc.stderr, file=sys.stderr)
        return 1

    print(f"Checked {len(stmts)} `use force::` statement(s) in guide rust fences.")
    for idx, msg in sorted(bad.items()):
        doc, line, stmt = stmts[idx]
        print(f"  FAIL {doc}:{line}: {stmt}\n       {msg}")
    print(f"{len(bad)} broken import(s).")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
