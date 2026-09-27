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
# Fragments compile under *exactly* the `force` feature(s) their own page
# documents (grouped by SURFACES_FRAGMENT_FEATURES in onramp_snippets.py),
# not under a single blanket `--features all` -- otherwise a fragment could
# pass only because some *other* surface's feature happened to be enabled
# too (Codex review on #1445: `composite_graph` compiled clean under `all`
# even though composite.md's own `force = { features = ["composite"] }`
# block doesn't mention `composite_graph`). The 3 sibling-crate fragments
# (force-pubsub/force-lake/force-marketingcloud) aren't gated by any
# `force` feature, so they're compiled in a second, separate pass that
# includes those sibling crates instead.
#
# Exit code is non-zero if any fragment fails to `cargo check`.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OUT_DIR="${REPO_ROOT}/target/onramp"
FRAGMENTS_DIR="${OUT_DIR}/surfaces_fragments"
CORE_PROJECT_DIR="${OUT_DIR}/surfaces_compile_project_core"
SIBLING_PROJECT_DIR="${OUT_DIR}/surfaces_compile_project_sibling"

rm -rf "${FRAGMENTS_DIR}" "${CORE_PROJECT_DIR}" "${SIBLING_PROJECT_DIR}"
mkdir -p "${FRAGMENTS_DIR}" "${CORE_PROJECT_DIR}/src/bin" "${SIBLING_PROJECT_DIR}/src/bin"

python3 "${REPO_ROOT}/scripts/onramp/onramp_snippets.py" extract-surfaces-fragments --out "${FRAGMENTS_DIR}"

MANIFEST="${FRAGMENTS_DIR}/MANIFEST.tsv"
if [ ! -s "${MANIFEST}" ]; then
    echo "No surfaces fragments extracted -- nothing to check."
    exit 0
fi

common_deps() {
    cat <<EOF
tokio = { version = "1", features = ["full"] }
tokio-stream = "0.1"
futures = "0.3"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
anyhow = "1.0"
EOF
}

cat > "${CORE_PROJECT_DIR}/Cargo.toml" <<EOF
[package]
name = "onramp-surfaces-fragments-core"
version = "0.0.0"
publish = false
edition = "2021"

# Detach from the enclosing workspace -- this project lives under target/
# purely as scratch space for the fragment compile check, it is not a member.
[workspace]

[dependencies]
# default-features = false: each cargo check invocation below turns on
# exactly the feature(s) its group's fragments document, via
# --features force/<name>[,force/<name>...] -- never the crate's own
# "rest" default and never "all".
force = { path = "${REPO_ROOT}/crates/force", default-features = false }
$(common_deps)
EOF

cat > "${SIBLING_PROJECT_DIR}/Cargo.toml" <<EOF
[package]
name = "onramp-surfaces-fragments-sibling"
version = "0.0.0"
publish = false
edition = "2021"

[workspace]

[dependencies]
# Sibling-crate fragments aren't gated by any \`force\` feature (they use
# only unconditional core modules: client/auth/session); "all" here is
# just a convenient default, not the thing under test.
force = { path = "${REPO_ROOT}/crates/force", features = ["all"] }
force-pubsub = { path = "${REPO_ROOT}/crates/force-pubsub" }
force-lake = { path = "${REPO_ROOT}/crates/force-lake" }
force-marketingcloud = { path = "${REPO_ROOT}/crates/force-marketingcloud" }
$(common_deps)
EOF

core_count=0
sibling_count=0
declare -A group_bins   # features -> space-separated bin names

while IFS=$'\t' read -r bin_name fragment_name features doc_path; do
    [ -z "${bin_name}" ] && continue
    if [ "${features}" = "SIBLING_CRATE" ]; then
        cp "${FRAGMENTS_DIR}/${bin_name}.rs" "${SIBLING_PROJECT_DIR}/src/bin/${bin_name}.rs"
        sibling_count=$((sibling_count + 1))
    else
        cp "${FRAGMENTS_DIR}/${bin_name}.rs" "${CORE_PROJECT_DIR}/src/bin/${bin_name}.rs"
        group_bins["${features}"]="${group_bins["${features}"]:-} ${bin_name}"
        core_count=$((core_count + 1))
    fi
done < "${MANIFEST}"

overall_status=0

if [ "${core_count}" -gt 0 ]; then
    echo "Compiling ${core_count} core surfaces fragment(s) in $((${#group_bins[@]})) feature group(s) (each against exactly its page's documented \`force\` feature(s))..."
    for features in "${!group_bins[@]}"; do
        bin_args=()
        for bin_name in ${group_bins["${features}"]}; do
            bin_args+=(--bin "${bin_name}")
        done

        feature_args=()
        label="(no optional feature)"
        if [ "${features}" != "NO_FEATURE" ]; then
            spec=""
            IFS=',' read -ra feats <<< "${features}"
            for feat in "${feats[@]}"; do
                spec="${spec:+${spec},}force/${feat}"
            done
            feature_args=(--features "${spec}")
            label="--features ${spec}"
        fi

        echo "  group ${label}: ${group_bins["${features}"]# }"
        if ! (cd "${CORE_PROJECT_DIR}" && cargo check --quiet "${feature_args[@]}" "${bin_args[@]}"); then
            overall_status=1
        fi
    done
fi

if [ "${sibling_count}" -gt 0 ]; then
    echo "Compiling ${sibling_count} sibling-crate fragment(s) (force-pubsub/force-lake/force-marketingcloud)..."
    if ! (cd "${SIBLING_PROJECT_DIR}" && cargo check --quiet --bins); then
        overall_status=1
    fi
fi

if [ "${overall_status}" -eq 0 ]; then
    echo "OK: all surfaces fragments compile under their documented feature(s)."
    exit 0
else
    echo "FAIL: one or more surfaces fragments do not compile. See errors above."
    echo "Manifest of extracted fragments -> onramp-fragment name -> features -> source doc:"
    cat "${MANIFEST}" 2>/dev/null || true
    exit 1
fi
