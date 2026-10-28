#!/usr/bin/env bash
# Onramp snippet CI, journey #5 (upgrade): compiles every "after" ```rust
# fragment in docs/guide/06-upgrading.md -- the page CHANGELOG.md's own
# 0.4.0 entry links to for a before/after fix on each of its three
# semver-breaking changes.
#
# Like the auth-flow guide (journey #2) and unlike README.md, these
# fragments are not full programs (no `fn main`) and reference a
# caller-supplied `client`, so `onramp_snippets.py extract-upgrade-fragments`
# wraps each one with a small stub preamble before cargo-checking it. That
# catches the same drift class the other onramp harnesses catch -- a
# renamed field, a changed return type -- for the one page in the corpus
# whose job is specifically to describe an API shape change; if the "after"
# fragment stops compiling, the guide itself is now wrong about how to fix
# the thing it says broke.
#
# The page's "before" snippets (old-shape code, meant to demonstrate what no
# longer compiles) are deliberately fenced ```rust,ignore, not ```rust, so
# they are invisible to this harness by construction -- there is nothing to
# cargo-check them against without vendoring the previous release.
#
# Exit code is non-zero if any "after" fragment fails to `cargo check`.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OUT_DIR="${REPO_ROOT}/target/onramp"
FRAGMENTS_DIR="${OUT_DIR}/upgrade_fragments"
PROJECT_DIR="${OUT_DIR}/upgrade_compile_project"

rm -rf "${FRAGMENTS_DIR}" "${PROJECT_DIR}"
mkdir -p "${FRAGMENTS_DIR}" "${PROJECT_DIR}/src/bin"

python3 "${REPO_ROOT}/scripts/onramp/onramp_snippets.py" extract-upgrade-fragments --out "${FRAGMENTS_DIR}"

shopt -s nullglob
fragment_files=("${FRAGMENTS_DIR}"/fragment_*.rs)
if [ ${#fragment_files[@]} -eq 0 ]; then
    echo "No upgrade-guide fragments extracted -- nothing to check."
    exit 0
fi

for f in "${fragment_files[@]}"; do
    cp "$f" "${PROJECT_DIR}/src/bin/$(basename "$f")"
done

cat > "${PROJECT_DIR}/Cargo.toml" <<EOF
[package]
name = "onramp-upgrade-fragments"
version = "0.0.0"
publish = false
edition = "2021"

# Detach from the enclosing workspace -- this project lives under target/
# purely as scratch space for the fragment compile check, it is not a member.
[workspace]

[dependencies]
force = { path = "${REPO_ROOT}/crates/force", features = ["all"] }
tokio = { version = "1", features = ["full"] }
anyhow = "1.0"
EOF

echo "Compiling ${#fragment_files[@]} upgrade-guide fragment(s) against the workspace crate (path dependency, --features all)..."
if (cd "${PROJECT_DIR}" && cargo check --quiet --bins); then
    echo "OK: all upgrade-guide 'after' fragments compile."
    exit 0
else
    echo "FAIL: one or more upgrade-guide fragments do not compile. See errors above."
    echo "Manifest of extracted fragments -> onramp-fragment name:"
    cat "${FRAGMENTS_DIR}/MANIFEST.tsv" 2>/dev/null || true
    exit 1
fi
