#!/usr/bin/env bash
# Onramp snippet CI, journey #2 (first real integration): compiles every
# ```rust fragment in docs/guide/02-choosing-an-auth-flow.md -- the page
# 01-getting-started.md's own "Next:" link sends every reader to once the
# quickstart works.
#
# Unlike README.md's Quick Start, these fragments are not full programs
# (`extract-programs` / compile_readme_snippets.sh skips them by design --
# no `fn main`) and they reference variables the reader is expected to
# supply (`client_id`, `client_secret`, ...), so `onramp_snippets.py
# extract-auth-flow-fragments` wraps each one with a small stub preamble
# before cargo-checking it. That catches the same drift class as the README
# harness -- a renamed constructor, a reordered or added argument, a changed
# return type -- for the page that, until this harness existed, had zero
# mechanism protecting it.
#
# Exit code is non-zero if any fragment fails to `cargo check`.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OUT_DIR="${REPO_ROOT}/target/onramp"
FRAGMENTS_DIR="${OUT_DIR}/auth_flow_fragments"
PROJECT_DIR="${OUT_DIR}/auth_flow_compile_project"

rm -rf "${FRAGMENTS_DIR}" "${PROJECT_DIR}"
mkdir -p "${FRAGMENTS_DIR}" "${PROJECT_DIR}/src/bin"

python3 "${REPO_ROOT}/scripts/onramp/onramp_snippets.py" extract-auth-flow-fragments --out "${FRAGMENTS_DIR}"

shopt -s nullglob
fragment_files=("${FRAGMENTS_DIR}"/fragment_*.rs)
if [ ${#fragment_files[@]} -eq 0 ]; then
    echo "No auth-flow fragments extracted -- nothing to check."
    exit 0
fi

for f in "${fragment_files[@]}"; do
    cp "$f" "${PROJECT_DIR}/src/bin/$(basename "$f")"
done

cat > "${PROJECT_DIR}/Cargo.toml" <<EOF
[package]
name = "onramp-auth-flow-fragments"
version = "0.0.0"
publish = false
edition = "2021"

# Detach from the enclosing workspace -- this project lives under target/
# purely as scratch space for the fragment compile check, it is not a member.
[workspace]

[dependencies]
# "username_password" isn't in the "all" bundle by design (it's a deliberate
# speed bump on a deprecated flow, see CLAUDE.md) but the guide documents it,
# so it's added explicitly here.
force = { path = "${REPO_ROOT}/crates/force", features = ["all", "username_password"] }
force-marketingcloud = { path = "${REPO_ROOT}/crates/force-marketingcloud" }
tokio = { version = "1", features = ["full"] }
anyhow = "1.0"
EOF

echo "Compiling ${#fragment_files[@]} auth-flow fragment(s) against the workspace crate (path dependency, --features all)..."
if (cd "${PROJECT_DIR}" && cargo check --quiet --bins); then
    echo "OK: all auth-flow fragments compile."
    exit 0
else
    echo "FAIL: one or more auth-flow fragments do not compile. See errors above."
    echo "Manifest of extracted fragments -> onramp-fragment name:"
    cat "${FRAGMENTS_DIR}/MANIFEST.tsv" 2>/dev/null || true
    exit 1
fi
