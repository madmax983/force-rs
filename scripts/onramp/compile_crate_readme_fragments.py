#!/usr/bin/env python3
"""Onramp snippet CI, journey #4b (first run of a sibling crate): `cargo check`
every ```rust fence in the README of force-pubsub, force-lake and
force-marketingcloud against the workspace crates.

Those READMEs are what crates.io and docs.rs show as the front page of each
sibling crate, and nothing compiles them (no `#![doc = include_str!]`, and
the surfaces harness only covers docs/guide/surfaces/*.md).

Rules, so the harness measures the page and not the stub:
  * a fence with its own `fn main` (or hidden `# async fn run` wrapper) is
    compiled verbatim, with rustdoc `# ` hidden-line prefixes stripped;
  * any other fence is a fragment: it is wrapped in an `async fn` whose
    parameters are ONLY what the README text says the reader already has
    (`client`, a force `ForceClient`). A free variable or a `/* ... */`
    placeholder is a defect in the README.

Exit non-zero on any compile error; prints a per-fence table either way.
"""
from __future__ import annotations

import re
import shutil
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
CRATES = ["force-pubsub", "force-lake", "force-marketingcloud"]
OUT = REPO / "target" / "onramp" / "crate_readme_project"
FENCE = re.compile(r"```rust[^\n]*\n(.*?)```", re.S)

HEAD = "#![allow(unused, dead_code, unused_imports, unused_variables, clippy::all)]\n"
FRAGMENT_ARGS = "client: force::client::ForceClient<force::auth::ClientCredentials>"


def unhide(body: str) -> str:
    return "\n".join(
        line[2:] if line.startswith("# ") else ("" if line == "#" else line)
        for line in body.splitlines()
    )


def build() -> list[tuple[str, int, str]]:
    units = []
    shutil.rmtree(OUT, ignore_errors=True)
    (OUT / "src" / "bin").mkdir(parents=True)
    for crate in CRATES:
        readme = REPO / "crates" / crate / "README.md"
        for i, raw in enumerate(FENCE.findall(readme.read_text()), 1):
            name = f"{crate.replace('-', '_')}__{i}"
            body = unhide(raw)
            if "fn main" in body:
                src = HEAD + body
            elif "async fn run" in body:
                src = HEAD + body + "\nfn main() {}\n"
            else:
                src = (
                    HEAD
                    + f"async fn run({FRAGMENT_ARGS}) -> std::result::Result<(), Box<dyn std::error::Error>> {{\n"
                    + body
                    + "\nOk(())\n}\nfn main() {}\n"
                )
            (OUT / "src" / "bin" / f"{name}.rs").write_text(src)
            units.append((crate, i, name))
    return units


def main() -> int:
    units = build()
    (OUT / "Cargo.toml").write_text(
        f"""[package]
name = "onramp-crate-readme-fragments"
version = "0.0.0"
publish = false
edition = "2021"

[workspace]

[dependencies]
force = {{ path = "{REPO}/crates/force", features = ["all"] }}
tokio = {{ version = "1", features = ["full"] }}
anyhow = "1"
futures = "0.3"
force-pubsub = {{ path = "{REPO}/crates/force-pubsub" }}
force-lake = {{ path = "{REPO}/crates/force-lake" }}
force-marketingcloud = {{ path = "{REPO}/crates/force-marketingcloud" }}
"""
    )
    proc = subprocess.run(
        ["cargo", "check", "--bins", "--keep-going", "--message-format", "short", "--color", "never"],
        cwd=OUT, capture_output=True, text=True,
    )
    errors: dict[str, list[str]] = {}
    for line in proc.stderr.splitlines():
        m = re.match(r"src/bin/(\w+)\.rs:\d+:\d+: error(.*)", line)
        if m:
            errors.setdefault(m.group(1), []).append(m.group(2).strip())
    bad = 0
    for crate, i, name in units:
        errs = errors.get(name)
        if errs:
            bad += 1
            print(f"FAIL  {crate} README fence #{i}")
            for e in errs:
                print(f"        {e}")
        else:
            print(f"ok    {crate} README fence #{i}")
    print(f"\nChecked {len(units)} crate-README rust fence(s): {bad} do not compile.")
    if proc.returncode != 0 and not errors:
        print(proc.stderr, file=sys.stderr)
        return 2
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
