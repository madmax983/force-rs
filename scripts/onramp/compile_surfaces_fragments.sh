#!/usr/bin/env bash
# Onramp snippet CI, journey #4 (first real integration, per API surface):
# compiles every ```rust fragment across the 18 pages under
# docs/guide/surfaces/ -- the reference every "How do I use <surface>?"
# question lands on, and the page README.md's own surfaces table links
# every reader to once the quickstart works.
#
# Same mechanism as compile_auth_flow_fragments.sh: these fragments have no
# `fn main` (extract-programs skips them by design) and reference variables
# the reader is expected to already have in scope (`client`, a handler like
# `tooling`/`cpq`/`ae`, ids), so `onramp_snippets.py
# extract-surfaces-fragments` wraps each one with a small stub preamble
# before cargo-checking it. That catches the same drift class as the README
# and auth-flow harnesses -- a renamed handler method, a reordered or added
# argument, a changed return type -- across all 18 surfaces, none of which
# had any compile coverage before this harness existed.
#
# Exit code is non-zero if any fragment fails to `cargo check`.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OUT_DIR="${REPO_ROOT}/target/onramp"
FRAGMENTS_DIR="${OUT_DIR}/surfaces_fragments"
PROJECT_DIR="${OUT_DIR}/surfaces_compile_project"

rm -rf "${FRAGMENTS_DIR}" "${PROJECT_DIR}"
mkdir -p "${FRAGMENTS_DIR}" "${PROJECT_DIR}/src/bin"

python3 "${REPO_ROOT}/scripts/onramp/onramp_snippets.py" extract-surfaces-fragments --out "${FRAGMENTS_DIR}"

shopt -s nullglob
fragment_files=("${FRAGMENTS_DIR}"/fragment_*.rs)
if [ ${#fragment_files[@]} -eq 0 ]; then
    echo "No surfaces fragments extracted -- nothing to check."
    exit 0
fi

for f in "${fragment_files[@]}"; do
    cp "$f" "${PROJECT_DIR}/src/bin/$(basename "$f")"
done

cat > "${PROJECT_DIR}/Cargo.toml" <<EOF
[package]
name = "onramp-surfaces-fragments"
version = "0.0.0"
publish = false
edition = "2021"

# Detach from the enclosing workspace -- this project lives under target/
# purely as scratch space for the fragment compile check, it is not a member.
[workspace]

[dependencies]
force = { path = "${REPO_ROOT}/crates/force", features = ["all"] }
force-pubsub = { path = "${REPO_ROOT}/crates/force-pubsub" }
force-lake = { path = "${REPO_ROOT}/crates/force-lake" }
force-marketingcloud = { path = "${REPO_ROOT}/crates/force-marketingcloud" }
tokio = { version = "1", features = ["full"] }
tokio-stream = "0.1"
futures = "0.3"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
anyhow = "1.0"
EOF

echo "Compiling ${#fragment_files[@]} surfaces fragment(s) against the workspace crates (path dependencies, --features all)..."
if (cd "${PROJECT_DIR}" && cargo check --quiet --bins); then
    echo "OK: all surfaces fragments compile."
    exit 0
else
    echo "FAIL: one or more surfaces fragments do not compile. See errors above."
    echo "Manifest of extracted fragments -> onramp-fragment name -> source doc:"
    cat "${FRAGMENTS_DIR}/MANIFEST.tsv" 2>/dev/null || true
    exit 1
fi
