#!/usr/bin/env python3
"""Folio drift scan: paths in CLAUDE.md tree diagrams vs the repo.

For each fenced tree whose first line is a repo path ending in `/` (e.g.
`crates/force/src/`), rebuild every entry's full path from the box-drawing
indentation and require it to exist. Entries containing `*` are globs and must
match at least one path. CLAUDE.md is loaded into every agent session, so a
phantom path is a confidently-wrong answer. Exit 1 on drift.
"""
import pathlib
import re
import sys

root = pathlib.Path(__file__).resolve().parents[2]
text = (root / "CLAUDE.md").read_text()
entry = re.compile(r"^((?:[│ ]   )*)[├└]── ([^\s#]+)")
checked = 0
missing = []
blocks, cur = [], None
for ln in text.splitlines():
    if ln.startswith("```"):
        if cur is None:
            cur = []
        else:
            blocks.append(cur)
            cur = None
    elif cur is not None:
        cur.append(ln)
for lines in blocks:
    if not lines or not re.fullmatch(r"crates/[\w\-/]+/", lines[0].strip()):
        continue
    base = lines[0].strip().rstrip("/")
    stack = []
    for line in lines[1:]:
        m = entry.match(line)
        if not m:
            continue
        depth = len(m.group(1)) // 4
        name = m.group(2)
        del stack[depth:]
        if name.rstrip("/").endswith("...") or name == "...":
            continue
        rel = "/".join(stack + [name.rstrip("/")])
        stack.append(name.rstrip("/"))
        checked += 1
        hits = list(root.glob(f"{base}/{rel}")) if "*" in rel else (
            [root / base / rel] if (root / base / rel).exists() else []
        )
        if not hits:
            missing.append(f"{base}/{rel}")
print("# Folio CLAUDE.md tree drift scan")
print(f"# tree entries checked: {checked}")
for p in missing:
    print(f"DRIFT: CLAUDE.md names {p}, not found")
print(f"# drift found: {len(missing)}")
sys.exit(1 if missing else 0)
