#!/usr/bin/env bash
# Onramp snippet CI, journey #3 (first real integration): compiles every
# ```rust fence (concatenated per page) in docs/guide/surfaces/*.md and docs/guide/03-operations.md --
# the per-API pages a developer opens right after the quickstart works, to
# do their real task. Each fence is wrapped in a shared stub (authenticated
# `client`, `Account`, `MyType`; see onramp_snippets.py) and `cargo check`ed
# against the workspace crate with every feature on, so a renamed method, a
# changed argument type, or a missing `use` line fails here instead of in a
# newcomer's editor.
#
# `--keep-going` so one broken fence doesn't hide the rest: the failing
# fence list is the defect count.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OUT_DIR="${REPO_ROOT}/target/onramp"
FRAGMENTS_DIR="${OUT_DIR}/surface_fragments"
PROJECT_DIR="${OUT_DIR}/surface_compile_project"

rm -rf "${FRAGMENTS_DIR}" "${PROJECT_DIR}"
mkdir -p "${FRAGMENTS_DIR}" "${PROJECT_DIR}/src/bin"

python3 "${REPO_ROOT}/scripts/onramp/onramp_snippets.py" extract-surface-fragments --out "${FRAGMENTS_DIR}"

for f in "${FRAGMENTS_DIR}"/page_*.rs; do
    cp "$f" "${PROJECT_DIR}/src/bin/$(basename "$f")"
done

cat > "${PROJECT_DIR}/Cargo.toml" <<EOF
[package]
name = "onramp-surface-fragments"
version = "0.0.0"
publish = false
edition = "2021"

# Detached from the enclosing workspace: scratch space under target/.
[workspace]

[dependencies]
force = { path = "${REPO_ROOT}/crates/force", features = ["all", "username_password"] }
force-marketingcloud = { path = "${REPO_ROOT}/crates/force-marketingcloud" }
force-pubsub = { path = "${REPO_ROOT}/crates/force-pubsub" }
force-lake = { path = "${REPO_ROOT}/crates/force-lake" }
tokio = { version = "1", features = ["full"] }
anyhow = "1.0"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
futures = "0.3"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
EOF

total=$(grep -c . "${FRAGMENTS_DIR}/MANIFEST.tsv")
echo "Compiling ${total} surface/operations page(s) against the workspace crate (--features all)..."
log="${OUT_DIR}/surface_compile.log"
if (cd "${PROJECT_DIR}" && cargo check --quiet --bins --keep-going --message-format short) >"${log}" 2>&1; then
    echo "OK: all ${total} surface/operations pages compile."
    exit 0
fi
grep -E '^src/bin/.*error' "${log}" || cat "${log}"
failed=$(grep -E '^src/bin/.*error' "${log}" | sed -E 's#^src/bin/([^:]+):.*#\1#' | sort -u || true)
n=$(printf '%s\n' "${failed}" | grep -c . || true)
echo "FAIL: ${n} of ${total} page(s) do not compile (full log: ${log})."
echo "Failing pages (page_<doc>):"
printf '%s\n' "${failed}"
exit 1
