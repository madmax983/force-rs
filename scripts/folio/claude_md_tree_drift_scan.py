#!/usr/bin/env python3
"""Folio drift scan: file names in CLAUDE.md tree diagrams vs the repo.

Every `├── name.rs` / `└── name.rs` entry in CLAUDE.md must exist somewhere
under crates/ (the module tree) or crates/force/tests (the test tree).
CLAUDE.md is loaded into every agent session, so a phantom file is a
confidently-wrong answer. Exit 1 on drift.
"""
import pathlib
import re
import sys

root = pathlib.Path(__file__).resolve().parents[2]
text = (root / "CLAUDE.md").read_text()
names = sorted(set(re.findall(r"[├└]──\s+([\w.\-]+\.rs)\b", text)))
have = {p.name for p in (root / "crates").rglob("*.rs")}
missing = [n for n in names if n not in have]
print("# Folio CLAUDE.md tree drift scan")
print(f"# file entries checked: {len(names)}")
for n in missing:
    print(f"DRIFT: CLAUDE.md names {n}, not found under crates/")
print(f"# drift found: {len(missing)}")
sys.exit(1 if missing else 0)
