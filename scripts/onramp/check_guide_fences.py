#!/usr/bin/env python3
"""Onramp snippet CI: every ```rust fence in docs/guide/03-operations.md and
docs/guide/surfaces/*.md must type-check.

check_guide_imports.py only proves `use force::...;` lines resolve, and the
fragment harnesses only cover README / 01 / 02 / 06. Everything a reader copies
from a surface page after the import line -- a constructor argument type, a
missing trait import, a stale method name -- was never compiled.

How it works: each fence becomes `mod fN { ... pub async fn run(client) { <fence> } }`
in a scratch crate (path dep on the workspace `force`, `--features all`). A
reader meets a surface page top to bottom, so `use` lines from EARLIER fences
on the same page are in scope for later ones (later pages' imports are not).
Free variables the prose supplies (`auth`, `records`, ...) are stubbed by
STUBS (global) + DOC_STUBS (per page); fences that shadow them win. Errors map
back to doc:line.

Usage: python3 scripts/onramp/check_guide_fences.py   (exit 1 on any failure)
"""
from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
OUT = REPO_ROOT / "target" / "onramp" / "guide_fences"
FENCE_RE = re.compile(r"```rust\n(.*?)```", re.DOTALL)
USE_RE = re.compile(r"^[ \t]*(use [^;]*;)", re.MULTILINE)

# Pages with fences that cannot be built in this scratch crate, and why.
EXCLUDED = {
    "docs/guide/surfaces/sibling-crates.md": "documents separate crates (force-pubsub/-lake/-marketingcloud), not `force`",
}

# Free variables the surrounding prose supplies. Injected as `let`s at the top
# of every fence's fn; a fence that declares its own binding simply shadows it.
STUBS = """
let auth = force::auth::ClientCredentials::new_production("id", "secret");
let soql = "SELECT Id FROM Account";
let body = serde_json::json!({});
let request_body = serde_json::json!({});
let cpq = client.cpq();
let tooling = client.tooling();
let ui = client.ui();
let ae = client.account_engagement("0Uv000000000001AAA");
let mut contact: force::types::DynamicSObject =
    serde_json::from_value(serde_json::json!({"attributes": {"type": "Contact"}})).expect("stub");
let file_bytes: Vec<u8> = Vec::new();
let content_document_id = "069000000000001AAA";
let account_id = "001000000000001AAA";
let request = force::api::graphql::GraphqlRequest::new("query { x }");
let force_client = client;
"""
# Per-page stubs/items, keyed by repo-relative path.
DOC_STUBS = {
    "docs/guide/surfaces/bulk.md": "let records: Vec<Account> = Vec::new();",
    "docs/guide/surfaces/soap.md": "let records: Vec<force::api::soap::SObject> = Vec::new();",
}
# Page-level return type of the generated fn (default anyhow::Result<()>).
DOC_RET = {
    "docs/guide/surfaces/soap.md": "force::error::Result<()>",
}
ITEMS = """
#![allow(unused, clippy::all)]
use serde::{Deserialize, Serialize};
use serde_json::json;
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
struct Account {}
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
struct MyResponse {}
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
struct MyRecord {}
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
struct MyType {}
"""


def docs() -> list[Path]:
    ops = REPO_ROOT / "docs" / "guide" / "03-operations.md"
    return [ops, *sorted((REPO_ROOT / "docs" / "guide" / "surfaces").glob("*.md"))]


def main() -> int:
    mods: list[str] = []
    manifest: list[tuple[str, int]] = []  # (doc, doc line) per module index
    for doc in docs():
        rel = str(doc.relative_to(REPO_ROOT))
        if rel in EXCLUDED:
            continue
        text = doc.read_text()
        seen_uses: list[str] = []
        for fence in FENCE_RE.finditer(text):
            body = fence.group(1)
            line = text.count("\n", 0, fence.start(1)) + 1
            idx = len(manifest)
            manifest.append((rel, line))
            own = {" ".join(m.group(1).split()) for m in USE_RE.finditer(body)}
            uses = " ".join(u for u in seen_uses if u not in own)
            ret = DOC_RET.get(rel, "anyhow::Result<()>")
            stubs = STUBS + DOC_STUBS.get(rel, "")
            # One line per fence header so the fence body keeps its own line
            # numbers: body line k is lib.rs line (module start + 1 + k).
            mods.append(
                f"mod f{idx} {{ use super::*; {uses}\n"
                f"pub async fn run(client: &force::client::ForceClient<force::auth::ClientCredentials>) -> {ret} {{\n"
                f"{stubs}\n#[allow(unreachable_code)] let _r: {ret} = async {{\n"
                f"{body}\n#[allow(unreachable_code)] Ok(()) }}.await; _r }} }}\n"
            )
            seen_uses += [u for u in sorted(own) if u not in seen_uses]

    (OUT / "src").mkdir(parents=True, exist_ok=True)
    (OUT / "Cargo.toml").write_text(
        f'[package]\nname = "onramp-guide-fences"\nversion = "0.0.0"\npublish = false\n'
        f'edition = "2021"\n\n[workspace]\n\n[dependencies]\n'
        f'force = {{ path = "{REPO_ROOT}/crates/force", features = ["all", "username_password"] }}\n'
        'anyhow = "1"\nserde = { version = "1", features = ["derive"] }\nserde_json = "1"\n'
        'tokio = { version = "1", features = ["full"] }\nfutures = "0.3"\ntracing = "0.1"\n'
        'tracing-subscriber = { version = "0.3", features = ["env-filter"] }\n'
    )
    lib = ITEMS + "\n" + "".join(mods)
    (OUT / "src" / "lib.rs").write_text(lib)

    starts = [(n, int(m.group(1))) for n, l in enumerate(lib.split("\n"), 1) if (m := re.match(r"mod f(\d+) ", l))]

    proc = subprocess.run(
        ["cargo", "check", "--keep-going", "--color", "never", "--message-format", "short"],
        cwd=OUT, capture_output=True, text=True,
    )
    bad: dict[int, list[str]] = {}
    for m in re.finditer(r"src/lib\.rs:(\d+):\d+: error(?:\[\w+\])?: (.*)", proc.stderr):
        ln = int(m.group(1))
        owner = max((s for s in starts if s[0] <= ln), default=None, key=lambda s: s[0])
        if owner is not None:
            bad.setdefault(owner[1], []).append(m.group(2)[:160])
    if proc.returncode != 0 and not bad:
        print(proc.stderr, file=sys.stderr)
        return 1

    print(f"Checked {len(manifest)} ```rust fence(s) in 03-operations.md + surfaces/*.md "
          f"({len(EXCLUDED)} page(s) excluded).")
    for idx, msgs in sorted(bad.items()):
        doc, line = manifest[idx]
        print(f"  FAIL {doc}:{line}")
        for msg in dict.fromkeys(msgs):
            print(f"       {msg}")
    print(f"{len(bad)} fence(s) do not compile.")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
