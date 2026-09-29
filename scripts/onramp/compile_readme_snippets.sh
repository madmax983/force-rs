#!/usr/bin/env bash
# Onramp snippet CI: compiles every runnable ```rust fence in the first-run
# docs (README.md, docs/adr/004-feature-gates.md, and
# docs/guide/01-getting-started.md) against the local workspace crate.
#
# This is the fast, network-free half of the Onramp harness: it catches API
# drift (missing imports, renamed methods, changed signatures) the moment a
# PR lands, using a path-dependency so it never touches crates.io. It does
# NOT catch a stale version pin in the docs (e.g. `force = "0.1"` after a
# release) -- that is `check_version_pins.sh`'s job. Run both.
#
# Exit code is non-zero if any snippet fails to `cargo check`.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OUT_DIR="${REPO_ROOT}/target/onramp"
SNIPPETS_DIR="${OUT_DIR}/snippets"
PROJECT_DIR="${OUT_DIR}/compile_project"

rm -rf "${SNIPPETS_DIR}" "${PROJECT_DIR}"
mkdir -p "${SNIPPETS_DIR}" "${PROJECT_DIR}/src/bin"

python3 "${REPO_ROOT}/scripts/onramp/onramp_snippets.py" extract-programs --out "${SNIPPETS_DIR}"

shopt -s nullglob
snippet_files=("${SNIPPETS_DIR}"/snippet_*.rs)
if [ ${#snippet_files[@]} -eq 0 ]; then
    echo "No runnable snippets extracted -- nothing to check."
    exit 0
fi

for f in "${snippet_files[@]}"; do
    cp "$f" "${PROJECT_DIR}/src/bin/$(basename "$f")"
done

cat > "${PROJECT_DIR}/Cargo.toml" <<EOF
[package]
name = "onramp-readme-snippets"
version = "0.0.0"
publish = false
edition = "2021"

# Detach from the enclosing workspace -- this project lives under target/
# purely as scratch space for the snippet compile check, it is not a member.
[workspace]

[dependencies]
force = { path = "${REPO_ROOT}/crates/force", features = ["all"] }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow = "1.0"
futures = "0.3"
EOF

echo "Compiling ${#snippet_files[@]} README snippet(s) against the workspace crate (path dependency, --features all)..."
if (cd "${PROJECT_DIR}" && cargo check --quiet --bins); then
    echo "OK: all README snippets compile."
    exit 0
else
    echo "FAIL: one or more README snippets do not compile. See errors above."
    echo "Manifest of extracted snippets -> source doc:"
    cat "${SNIPPETS_DIR}/MANIFEST.tsv" 2>/dev/null || true
    exit 1
fi
