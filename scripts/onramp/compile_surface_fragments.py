#!/usr/bin/env python3
"""Onramp snippet CI, journey #3 (first real integration): `cargo check` every
```rust fence in docs/guide/surfaces/*.md against the workspace crate.

Those per-surface pages are where a developer lands after the quickstart
("hello world -> my use case"). check_guide_imports.py only proves their
`use force::...` lines resolve; nothing proved the *calls* match the real
signatures, so a page could tell a reader to pass a `&str` where the method
takes a `SoqlQueryBuilder` and CI stayed green.

Each fence becomes its own `async fn` (its own scope, exactly how a reader
pastes it) preceded by a stub preamble. The preamble defines ONLY values the
page text itself says the reader supplies (`records`, `request_body`, ...);
anything else unresolved is a defect in the page. Fences whose body is
deliberately not compilable carry no marker support here by design -- if a
fence cannot compile, fence it ```text or ```rust,ignore in the page.

Exit non-zero on any compile error; prints a per-fence table either way.
"""
from __future__ import annotations

import re
import shutil
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
SURFACES = REPO / "docs" / "guide" / "surfaces"
OUT = REPO / "target" / "onramp" / "surface_project"

HEAD = """#![allow(unused, dead_code, unused_imports, unused_variables, clippy::all)]
use force::api::RestOperation;
use serde_json::json;
#[derive(serde::Serialize, serde::Deserialize, Default, Clone, Debug)]
struct Account {}
#[derive(serde::Serialize, serde::Deserialize, Default, Clone, Debug)]
struct MyType {}
#[derive(serde::Serialize, serde::Deserialize, Default, Clone, Debug)]
struct MyRecord {}
#[derive(serde::Serialize, serde::Deserialize, Default, Clone, Debug)]
struct MyResponse {}
"""

# Values each page's prose says the reader supplies, plus handles that earlier
# fences on the same page define (`let ui = client.ui();`).
PREAMBLES: dict[str, str] = {
    "apex-rest": "let request_body = json!({}); let body = json!({});",
    "files": "let file_bytes: Vec<u8> = vec![]; let content_document_id = \"069000000000001AAA\"; let account_id = \"001000000000001AAA\";",
    "graphql": "let request = force::api::graphql::GraphqlRequest::new(\"{ uiapi { query { Account { edges { node { Id } } } } } }\");",
    "ui": "let ui = client.ui();",
    "bulk": "let records: Vec<Account> = vec![];",
    "data-utility": "let mut contact: force::types::DynamicSObject = serde_json::from_value(json!({})).unwrap();",
    "cpq": "let cpq = client.cpq();",
    "tooling": "let tooling = client.tooling();",
    "account-engagement": "let ae = client.account_engagement(\"0Uv000000000001AAA\");",
    "data-cloud": "let auth = force::auth::ClientCredentials::new(\"client_id\", \"client_secret\", \"https://login.salesforce.com/services/oauth2/token\");",
    "sibling-crates": "let force_client = &client;",
    "soap": "let records: Vec<force::api::soap::SObject> = vec![];",
}

# Pages whose calls return a sibling-crate error type, not ForceError.
BOXED_ERR = {"sibling-crates"}

# Fences that cannot be a plain async fn body. Each needs a reason.
SKIP: dict[str, str] = {}

FENCE = re.compile(r"```rust\n(.*?)```", re.S)


def build() -> list[tuple[str, int, str]]:
    units = []
    shutil.rmtree(OUT, ignore_errors=True)
    (OUT / "src" / "bin").mkdir(parents=True)
    for page in sorted(SURFACES.glob("*.md")):
        stem = page.stem
        if stem in SKIP:
            print(f"SKIP  {stem}: {SKIP[stem]}")
            continue
        for i, body in enumerate(FENCE.findall(page.read_text()), 1):
            name = f"{stem.replace('-', '_')}__{i}"
            ret = "std::result::Result<(), Box<dyn std::error::Error>>" if stem in BOXED_ERR else "force::error::Result<()>"
            src = (
                HEAD
                + f"async fn run(client: force::client::ForceClient<force::auth::ClientCredentials>) -> {ret} {{\n"
                + PREAMBLES.get(stem, "")
                + "\n"
                + body
                + "\nOk(())\n}\nfn main() {}\n"
            )
            (OUT / "src" / "bin" / f"{name}.rs").write_text(src)
            units.append((stem, i, name))
    return units


def main() -> int:
    units = build()
    (OUT / "Cargo.toml").write_text(
        f"""[package]
name = "onramp-surface-fragments"
version = "0.0.0"
publish = false
edition = "2021"

[workspace]

[dependencies]
force = {{ path = "{REPO}/crates/force", features = ["all"] }}
tokio = {{ version = "1", features = ["full"] }}
anyhow = "1"
serde = {{ version = "1", features = ["derive"] }}
serde_json = "1"
futures = "0.3"
force-pubsub = {{ path = "{REPO}/crates/force-pubsub" }}
force-lake = {{ path = "{REPO}/crates/force-lake" }}
force-marketingcloud = {{ path = "{REPO}/crates/force-marketingcloud" }}
"""
    )
    env_args = ["cargo", "check", "--bins", "--keep-going", "--message-format", "short", "--color", "never"]
    proc = subprocess.run(env_args, cwd=OUT, capture_output=True, text=True)
    errors: dict[str, list[str]] = {}
    for line in proc.stderr.splitlines():
        m = re.match(r"src/bin/(\w+)\.rs:\d+:\d+: error(.*)", line)
        if m:
            errors.setdefault(m.group(1), []).append(m.group(2).strip())
    bad = 0
    for stem, i, name in units:
        errs = errors.get(name)
        if errs:
            bad += 1
            print(f"FAIL  {stem} fence #{i}")
            for e in errs:
                print(f"        {e}")
    print(f"\nChecked {len(units)} surface-guide rust fence(s): {bad} do not compile.")
    if proc.returncode != 0 and not errors:
        print(proc.stderr, file=sys.stderr)
        return 2
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
