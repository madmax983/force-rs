#!/usr/bin/env python3
"""Folio corpus audit: internal link check + reachability-from-root orphan scan.

Two defect shapes, both accuracy/findability rather than opinion:
  * BROKEN link  - a relative markdown link whose target does not exist on
                    disk. A dead link gives a reader false navigational
                    confidence; unlike missing content, the reader cannot
                    detect it until they click.
  * ORPHAN page  - a page in the corpus that is unreachable by clicking
                    links starting from the corpus's real entry points
                    (README.md, docs/README.md, CHANGELOG.md). A page with
                    inbound links only from *other* orphaned pages is still
                    unreachable from where a reader actually starts, so this
                    does a root-reachability BFS rather than a raw inbound-
                    link count.

Scope: docs/**/*.md, README.md, CHANGELOG.md (the reader-facing corpus this
project's Folio agent owns). Not scoped: crates/*/README.md (crate-internal),
rustdoc comments (covered by `cargo doc`), examples/*.rs (covered by
scripts/onramp's snippet harness).

Usage:
    python3 scripts/folio/link_orphan_scan.py
Exit status is nonzero if any broken link or any orphan page is found, so
this can gate CI: an orphan is exactly the kind of regression this harness
exists to catch (see the docs/README.md fix this harness was built for), so
a page that goes unreachable again must fail the job, not just print.
"""
from __future__ import annotations

import re
import sys
from collections import deque
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
CORPUS_GLOBS = ["docs/**/*.md", "README.md", "CHANGELOG.md"]
ENTRY_POINTS = ["README.md", "docs/README.md", "CHANGELOG.md"]

# Inline links: [text](target). Reference-style links: [label]: target, the
# form used for e.g. `[`RestOperation`]: ../../../crates/.../rest_operation.rs`
# in the surface guides and `[ADR-025]: 025-username-password-auth.md` in the
# ADRs. Both forms are load-bearing in this corpus and must be checked.
INLINE_LINK_RE = re.compile(r"\[([^\]]*)\]\(([^)]+)\)")
REFERENCE_LINK_RE = re.compile(r"^[ ]{0,3}\[[^\]]+\]:\s*<?([^\s>]+)>?", re.MULTILINE)


def iter_link_targets(text: str) -> list[str]:
    targets = [m.group(2) for m in INLINE_LINK_RE.finditer(text)]
    targets += [m.group(1) for m in REFERENCE_LINK_RE.finditer(text)]
    return targets


def is_external(target: str) -> bool:
    return target.startswith(("http://", "https://", "mailto:"))


def corpus_files() -> list[Path]:
    files: list[Path] = []
    for glob in CORPUS_GLOBS:
        files.extend(REPO_ROOT.glob(glob))
    return sorted(set(f for f in files if f.is_file()))


def resolve(file_path: Path, target: str) -> Path:
    target_path = target.split("#", 1)[0]
    if target_path == "":
        return file_path
    if target_path.startswith("/"):
        candidate = REPO_ROOT / target_path.lstrip("/")
    else:
        candidate = (file_path.parent / target_path).resolve()
    if candidate.is_dir():
        # GitHub (and most doc renderers) resolve a directory link to its README.
        candidate = candidate / "README.md"
    return candidate


def main() -> int:
    files = corpus_files()
    rel_files = set(f.relative_to(REPO_ROOT) for f in files)

    edges: dict[Path, set[Path]] = {f: set() for f in rel_files}
    broken: list[tuple[str, str]] = []
    total_links = 0

    for f in files:
        rel_f = f.relative_to(REPO_ROOT)
        text = f.read_text(encoding="utf-8", errors="replace")
        for raw_target in iter_link_targets(text):
            target = raw_target.strip()
            if is_external(target) or target.startswith("#"):
                continue
            total_links += 1
            candidate = resolve(f, target)
            if not candidate.exists():
                broken.append((str(rel_f), target))
                continue
            try:
                rel_c = candidate.relative_to(REPO_ROOT)
            except ValueError:
                continue
            if rel_c in rel_files:
                edges[rel_f].add(rel_c)

    roots = [Path(e) for e in ENTRY_POINTS if Path(e) in rel_files]
    reachable: set[Path] = set(roots)
    queue = deque(roots)
    while queue:
        cur = queue.popleft()
        for nxt in edges.get(cur, ()):
            if nxt not in reachable:
                reachable.add(nxt)
                queue.append(nxt)

    orphans = sorted(str(f) for f in rel_files if f not in reachable)

    print("# Folio corpus link check + reachability orphan scan")
    print(f"# corpus files: {len(files)}")
    print(f"# internal links checked: {total_links}")
    print(f"# broken links: {len(broken)}")
    for src, tgt in broken:
        print(f"  BROKEN  {src} -> {tgt}")
    print(f"# entry points: {[str(r) for r in roots]}")
    print(f"# reachable pages: {len(reachable)} / {len(rel_files)}")
    print(f"# unreachable (orphan) pages: {len(orphans)}")
    for o in orphans:
        print(f"  ORPHAN  {o}")

    return 1 if (broken or orphans) else 0


if __name__ == "__main__":
    sys.exit(main())
