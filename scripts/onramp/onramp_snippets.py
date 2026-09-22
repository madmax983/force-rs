#!/usr/bin/env python3
"""Onramp DX harness: extract and audit the Rust code fences in the docs a
newcomer actually lands on for the first-run journey (README.md and
docs/guide/01-getting-started.md).

This is the tool that keeps those pages honest. Nothing in `cargo test --doc`
touches them (they are not `include_str!`'d into any crate), so a fenced
code block here can drift out of sync with the published API forever without
any CI job noticing. This script gives them the same discipline as the
rest of the codebase:

  extract-programs   Pull every fenced ```rust block that looks like a full
                      program (has `fn main`) out into standalone .rs files
                      so they can be `cargo check`ed against the workspace.

  check-version-pins  Scan for `force = "X"` / `force = { version = "X", ...}`
                      dependency pins in docs and flag any that do not match
                      the workspace's own version. This is what should have
                      caught the 0.1 -> 0.4 drift.

  extract-quickstart  Pull the literal ```toml block under "## Installation"
                      and the literal ```rust block under "## Quick Start"
                      out of README.md, verbatim, for the clean-room harness
                      to build against the *published* crates.io artifact.

  extract-auth-flow-fragments
                      Pull each ```rust fragment out of
                      docs/guide/02-choosing-an-auth-flow.md -- the page
                      01-getting-started.md's own "Next:" link sends every
                      reader to -- and wrap it with a small stub preamble (so
                      free variables like `client_id` type-check) into a
                      standalone .rs file, for the same cargo-check drift
                      catch `extract-programs` gives README.md. These
                      fragments have no `fn main`, so `extract-programs`
                      skips them by design; unlike README.md they also
                      reference variables the reader is expected to supply
                      (credentials), so they need a preamble instead of
                      running verbatim.

Usage:
    python3 scripts/onramp/onramp_snippets.py extract-programs --out DIR [FILES...]
    python3 scripts/onramp/onramp_snippets.py check-version-pins [FILES...]
    python3 scripts/onramp/onramp_snippets.py extract-quickstart --out DIR
    python3 scripts/onramp/onramp_snippets.py extract-auth-flow-fragments --out DIR
"""
from __future__ import annotations

import argparse
import re
import sys
import textwrap
import tomllib
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
DEFAULT_DOC_FILES = [
    REPO_ROOT / "README.md",
    REPO_ROOT / "docs" / "adr" / "004-feature-gates.md",
    REPO_ROOT / "docs" / "guide" / "01-getting-started.md",
]
# Deliberately NOT included: docs/guide/surfaces/*.md. Those per-surface
# reference pages use `force = { version = "...", ... }` / `"*"` as an
# intentional "whatever you already pinned in Cargo.toml" placeholder (the
# reader is assumed to have already followed 01-getting-started.md), not a
# concrete version -- flagging them here would be a false positive on every
# release, not a real drift defect.

FENCE_RE = re.compile(r"```rust\n(.*?)```", re.DOTALL)
# Matches `force = "0.1"` and `force = { version = "0.1", ... }`
VERSION_PIN_RE = re.compile(
    r'force\s*=\s*(?:"(?P<bare>[^"]+)"'
    r'|\{[^}]*version\s*=\s*"(?P<inner>[^"]+)"[^}]*\})'
)

AUTH_FLOW_GUIDE = REPO_ROOT / "docs" / "guide" / "02-choosing-an-auth-flow.md"
# Matches a `<!-- onramp-fragment: NAME -->` marker immediately followed by
# the ```rust fence it names. The marker lives in the doc itself (not in a
# side-table here) so a reordered or deleted fragment can't silently go
# unchecked -- moving the fence without moving its marker is a `KeyError` at
# extraction time, not a silent skip.
FRAGMENT_RE = re.compile(r"<!-- onramp-fragment: (?P<name>[\w-]+) -->\n```rust\n(?P<body>.*?)```", re.DOTALL)

# Every fragment in docs/guide/02-choosing-an-auth-flow.md references
# variables the reader is expected to supply (their own credentials), so
# each one needs a small stub preamble before it type-checks standalone.
# Stub *values* are throwaway strings -- only the *types* matter for a
# cargo-check-level drift catch (renamed constructor, reordered/added
# argument, changed return type). Keyed by the `onramp-fragment:` marker
# name so a fragment with no matching entry fails extraction loudly instead
# of being skipped.
FRAGMENT_PREAMBLES: dict[str, str] = {
    "client_credentials": """
        let client_id = "stub-client-id";
        let client_secret = "stub-client-secret";
    """,
    "jwt_bearer": """
        let client_id = "stub-client-id";
    """,
    "auth_code_stages": """
        // Owned `String`s, not `&str`: the fragment borrows these
        // (`&login_url`, `&client_id`, `&redirect_uri`) for the authorize
        // URL, then moves the originals into `AuthorizationCode::new` --
        // `&&str` doesn't satisfy an `impl Into<String>` bound, `&String` does.
        let login_url = "https://login.salesforce.com".to_string();
        let client_id = "stub-client-id".to_string();
        let redirect_uri = "https://example.com/callback".to_string();
        let client_secret: Option<String> = None;
        let received_code = "stub-code";
        let token_url = "https://login.salesforce.com/services/oauth2/token";
    """,
    "auth_code_builder_shortcut": """
        use force::auth::PkceChallenge;
        use force::client::ForceClientBuilder;
        let client_id = "stub-client-id";
        let redirect_uri = "https://example.com/callback";
        let code = "stub-code";
        let token_url = "https://login.salesforce.com/services/oauth2/token";
        let pkce = PkceChallenge::generate();
    """,
    "username_password": """
        let client_id = "stub-client-id";
        let client_secret = "stub-client-secret";
        let password = "stub-password";
        let security_token = "stub-token";
    """,
    "data_cloud": """
        let client_id = "stub-client-id";
        let client_secret = "stub-client-secret";
    """,
    "marketing_cloud": """
        let client_id = "stub-client-id";
        let client_secret = "stub-client-secret";
    """,
    "agentforce": """
        let client_id = "stub-client-id";
        let client_secret = "stub-client-secret";
    """,
}


def workspace_version() -> str:
    cargo_toml = REPO_ROOT / "Cargo.toml"
    data = tomllib.loads(cargo_toml.read_text())
    return data["workspace"]["package"]["version"]


def extract_rust_fences(text: str) -> list[str]:
    return [m.group(1) for m in FENCE_RE.finditer(text)]


def looks_like_program(snippet: str) -> bool:
    return "fn main" in snippet


def cmd_extract_programs(args: argparse.Namespace) -> int:
    out_dir = Path(args.out)
    out_dir.mkdir(parents=True, exist_ok=True)
    files = [Path(f) for f in args.files] if args.files else DEFAULT_DOC_FILES

    count = 0
    manifest_lines = []
    for doc in files:
        if not doc.exists():
            continue
        text = doc.read_text()
        for snippet in extract_rust_fences(text):
            if not looks_like_program(snippet):
                continue
            count += 1
            out_file = out_dir / f"snippet_{count:03d}.rs"
            out_file.write_text(snippet)
            manifest_lines.append(f"{out_file.name}\t{doc.relative_to(REPO_ROOT)}")

    (out_dir / "MANIFEST.tsv").write_text("\n".join(manifest_lines) + "\n" if manifest_lines else "")
    print(f"Extracted {count} runnable snippet(s) from {len(files)} doc file(s) into {out_dir}")
    return 0


def cmd_check_version_pins(args: argparse.Namespace) -> int:
    files = [Path(f) for f in args.files] if args.files else DEFAULT_DOC_FILES
    expected = workspace_version()
    expected_major_minor = ".".join(expected.split(".")[:2])

    mismatches = []
    checked = 0
    for doc in files:
        if not doc.exists():
            continue
        text = doc.read_text()
        for lineno, line in enumerate(text.splitlines(), start=1):
            for m in VERSION_PIN_RE.finditer(line):
                pin = m.group("bare") or m.group("inner")
                checked += 1
                if pin != expected_major_minor and pin != expected:
                    mismatches.append((doc.relative_to(REPO_ROOT), lineno, pin, line.strip()))

    print(f"Checked {checked} `force` version pin(s) against workspace version {expected}.")
    if mismatches:
        print(f"\n{len(mismatches)} stale version pin(s) found:\n")
        for doc, lineno, pin, line in mismatches:
            print(f"  {doc}:{lineno}: pinned to \"{pin}\", workspace is {expected}")
            print(f"    {line}")
        return 1

    print("All version pins match the workspace version.")
    return 0


def section_after_heading(text: str, heading: str) -> str:
    """Return the text between `heading` and the next `##` heading."""
    idx = text.index(heading)
    rest = text[idx + len(heading):]
    next_heading = re.search(r"\n## ", rest)
    return rest[: next_heading.start()] if next_heading else rest


def first_fence(text: str, lang: str) -> str:
    m = re.search(rf"```{lang}\n(.*?)```", text, re.DOTALL)
    if not m:
        raise ValueError(f"no ```{lang} fence found")
    return m.group(1)


def cmd_extract_quickstart(args: argparse.Namespace) -> int:
    readme = REPO_ROOT / "README.md"
    text = readme.read_text()

    toml_block = first_fence(section_after_heading(text, "## Installation"), "toml")
    # The Installation fence shows the base deps, then a blank line, then a
    # commented-out alternative ("# Or enable specific features: ..."). Only
    # the first paragraph is the literal "add this" step; the rest is prose
    # illustrating an alternative, not a second statement to paste in too.
    toml_snippet = toml_block.split("\n\n", 1)[0]
    rust_snippet = first_fence(section_after_heading(text, "## Quick Start"), "rust")

    out_dir = Path(args.out)
    out_dir.mkdir(parents=True, exist_ok=True)
    (out_dir / "Cargo.deps.toml").write_text(toml_snippet)
    (out_dir / "main.rs").write_text(rust_snippet)
    print(f"Wrote verbatim Installation deps -> {out_dir / 'Cargo.deps.toml'}")
    print(f"Wrote verbatim Quick Start program -> {out_dir / 'main.rs'}")
    return 0


def cmd_extract_auth_flow_fragments(args: argparse.Namespace) -> int:
    out_dir = Path(args.out)
    out_dir.mkdir(parents=True, exist_ok=True)
    text = AUTH_FLOW_GUIDE.read_text()

    fragments = list(FRAGMENT_RE.finditer(text))
    if not fragments:
        print(f"No `<!-- onramp-fragment: ... -->` markers found in {AUTH_FLOW_GUIDE}", file=sys.stderr)
        return 1

    # FRAGMENT_RE only finds fences immediately preceded by a marker, so a
    # new ```rust fence added without one would otherwise be silently
    # skipped instead of failing loudly -- defeating the point of this
    # harness for exactly the fence that's newest and least reviewed. Count
    # every ```rust fence in the file independently and require the two
    # counts to match.
    all_rust_fences = len(extract_rust_fences(text))
    if all_rust_fences != len(fragments):
        print(
            f"{AUTH_FLOW_GUIDE} has {all_rust_fences} ```rust fence(s) but only "
            f"{len(fragments)} carry a `<!-- onramp-fragment: NAME -->` marker "
            "immediately above them. Every rust fence in this file must be "
            "marked (and registered in FRAGMENT_PREAMBLES) so it's covered by "
            "this harness -- an unmarked fence is invisible to it.",
            file=sys.stderr,
        )
        return 1

    seen_names: set[str] = set()
    manifest_lines = []
    for i, m in enumerate(fragments, start=1):
        name = m.group("name")
        if name in seen_names:
            print(f"Duplicate onramp-fragment name '{name}' in {AUTH_FLOW_GUIDE}", file=sys.stderr)
            return 1
        seen_names.add(name)

        if name not in FRAGMENT_PREAMBLES:
            print(
                f"No stub preamble registered for onramp-fragment '{name}' "
                f"(scripts/onramp/onramp_snippets.py: FRAGMENT_PREAMBLES). "
                "Every fragment must have one so it can type-check standalone.",
                file=sys.stderr,
            )
            return 1

        body = m.group("body")
        preamble = textwrap.dedent(FRAGMENT_PREAMBLES[name]).strip()
        program = (
            "#[tokio::main]\n"
            "async fn main() -> anyhow::Result<()> {\n"
            f"    {preamble}\n\n"
            f"{textwrap.indent(body, '    ')}\n"
            "    Ok(())\n"
            "}\n"
        )
        out_file = out_dir / f"fragment_{i:03d}_{name}.rs"
        out_file.write_text(program)
        manifest_lines.append(f"{out_file.name}\t{name}")

    (out_dir / "MANIFEST.tsv").write_text("\n".join(manifest_lines) + "\n")
    print(f"Extracted {len(fragments)} auth-flow fragment(s) from {AUTH_FLOW_GUIDE} into {out_dir}")

    unused = set(FRAGMENT_PREAMBLES) - seen_names
    if unused:
        print(
            f"Note: FRAGMENT_PREAMBLES has {len(unused)} entry(ies) with no matching "
            f"marker in the doc (stale after an edit?): {sorted(unused)}",
            file=sys.stderr,
        )
        return 1
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)

    p_extract = sub.add_parser("extract-programs")
    p_extract.add_argument("--out", required=True)
    p_extract.add_argument("files", nargs="*")
    p_extract.set_defaults(func=cmd_extract_programs)

    p_check = sub.add_parser("check-version-pins")
    p_check.add_argument("files", nargs="*")
    p_check.set_defaults(func=cmd_check_version_pins)

    p_quickstart = sub.add_parser("extract-quickstart")
    p_quickstart.add_argument("--out", required=True)
    p_quickstart.set_defaults(func=cmd_extract_quickstart)

    p_auth_fragments = sub.add_parser("extract-auth-flow-fragments")
    p_auth_fragments.add_argument("--out", required=True)
    p_auth_fragments.set_defaults(func=cmd_extract_auth_flow_fragments)

    args = parser.parse_args()
    return args.func(args)


if __name__ == "__main__":
    sys.exit(main())
