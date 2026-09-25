#!/usr/bin/env bash
# Onramp snippet CI, journey #3 (operating a client in production): compiles
# every ```rust fragment in docs/guide/03-operations.md -- the page every
# auth-flow section in 02-choosing-an-auth-flow.md points back to for the
# retry loop, and the page a reader reaches for a `ClientConfig`, the
# `ForceError` match arms, an API-version pin, or the tracing setup once
# something real breaks.
#
# Same mechanism as compile_auth_flow_fragments.sh: these are fragments, not
# full programs (no `fn main`), and they reference variables/imports the
# reader is expected to already have in scope from surrounding prose (a
# `client`, a `soql` string, an earlier `use`), so `onramp_snippets.py
# extract-operations-fragments` wraps each one with a small stub preamble
# before cargo-checking it. Until this harness existed, a renamed
# `ForceError`/`HttpError` variant, a `ClientConfig` field rename, or a typo
# in the tracing-subscriber snippet could drift silently forever -- nothing
# here is `include_str!`'d into any crate or touched by `cargo test --doc`.
#
# Exit code is non-zero if any fragment fails to `cargo check`.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OUT_DIR="${REPO_ROOT}/target/onramp"
FRAGMENTS_DIR="${OUT_DIR}/operations_fragments"
PROJECT_DIR="${OUT_DIR}/operations_compile_project"

rm -rf "${FRAGMENTS_DIR}" "${PROJECT_DIR}"
mkdir -p "${FRAGMENTS_DIR}" "${PROJECT_DIR}/src/bin"

python3 "${REPO_ROOT}/scripts/onramp/onramp_snippets.py" extract-operations-fragments --out "${FRAGMENTS_DIR}"

shopt -s nullglob
fragment_files=("${FRAGMENTS_DIR}"/fragment_*.rs)
if [ ${#fragment_files[@]} -eq 0 ]; then
    echo "No operations fragments extracted -- nothing to check."
    exit 0
fi

for f in "${fragment_files[@]}"; do
    cp "$f" "${PROJECT_DIR}/src/bin/$(basename "$f")"
done

cat > "${PROJECT_DIR}/Cargo.toml" <<EOF
[package]
name = "onramp-operations-fragments"
version = "0.0.0"
publish = false
edition = "2021"

# Detach from the enclosing workspace -- this project lives under target/
# purely as scratch space for the fragment compile check, it is not a member.
[workspace]

[dependencies]
force = { path = "${REPO_ROOT}/crates/force", features = ["all", "username_password"] }
tokio = { version = "1", features = ["full"] }
anyhow = "1.0"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
EOF

echo "Compiling ${#fragment_files[@]} operations fragment(s) against the workspace crate (path dependency, --features all)..."
if (cd "${PROJECT_DIR}" && cargo check --quiet --bins); then
    echo "OK: all operations fragments compile."
    exit 0
else
    echo "FAIL: one or more operations fragments do not compile. See errors above."
    echo "Manifest of extracted fragments -> onramp-fragment name:"
    cat "${FRAGMENTS_DIR}/MANIFEST.tsv" 2>/dev/null || true
    exit 1
fi
