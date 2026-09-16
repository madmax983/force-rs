#!/usr/bin/env bash
# Onramp clean-room harness: journey #1 (zero -> hello world).
#
# Starts from an empty `cargo new` project -- not this workspace -- and
# follows README.md's own "Installation" + "Quick Start" sections verbatim,
# byte for byte, against the *published* force crate on crates.io. This is
# the real journey an external developer takes: our own examples/ and tests/
# never exercise this path because they depend on `crates/force` by local
# `path`, so a version-pin or API-shape regression in the docs is invisible
# to every other CI job in this repository.
#
# Logs every step, the wall-clock time, and the exact failure point. Exit
# code is non-zero (and the log says which step) if the documented path does
# not compile.
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
WORK_DIR="$(mktemp -d /tmp/onramp-cleanroom.XXXXXX)"
LOG="${WORK_DIR}/cleanroom.log"
STEP=0
START_EPOCH=$(date +%s)

log() {
    echo "$@" | tee -a "${LOG}"
}

step() {
    STEP=$((STEP + 1))
    log ""
    log "== Step ${STEP}: $* =="
}

fail() {
    local elapsed=$(( $(date +%s) - START_EPOCH ))
    log ""
    log "RESULT: FAIL at step ${STEP} (${elapsed}s elapsed)"
    log "Reason: $*"
    log "Work dir preserved at: ${WORK_DIR}"
    echo "${STEP}" > "${WORK_DIR}/failed_step"
    exit 1
}

log "Onramp clean-room run started $(date -u +%Y-%m-%dT%H:%M:%SZ)"
log "Following README.md 'Installation' + 'Quick Start' verbatim, against crates.io."
log "Work dir: ${WORK_DIR}"

step "extract verbatim Installation/Quick Start snippets from README.md"
QS_DIR="${WORK_DIR}/quickstart_src"
if ! python3 "${REPO_ROOT}/scripts/onramp/onramp_snippets.py" extract-quickstart --out "${QS_DIR}" | tee -a "${LOG}"; then
    fail "could not extract snippets from README.md"
fi

step "cargo new a pristine project (simulates a developer with nothing installed but Rust)"
if ! (cd "${WORK_DIR}" && cargo new newdev --bin --quiet) >>"${LOG}" 2>&1; then
    fail "cargo new failed"
fi
PROJECT_DIR="${WORK_DIR}/newdev"

step "write Cargo.toml exactly as shown in README.md Installation section"
{
    echo "[package]"
    echo "name = \"newdev\""
    echo "version = \"0.1.0\""
    echo "edition = \"2021\""
    echo
    cat "${QS_DIR}/Cargo.deps.toml"
} > "${PROJECT_DIR}/Cargo.toml"
cat "${PROJECT_DIR}/Cargo.toml" >> "${LOG}"

step "write src/main.rs exactly as shown in README.md Quick Start section"
cp "${QS_DIR}/main.rs" "${PROJECT_DIR}/src/main.rs"

step "cargo build (this is what a newcomer runs to see if the example works)"
BUILD_START=$(date +%s)
if (cd "${PROJECT_DIR}" && cargo build 2>&1 | tee -a "${LOG}"); then
    BUILD_ELAPSED=$(( $(date +%s) - BUILD_START ))
    TOTAL_ELAPSED=$(( $(date +%s) - START_EPOCH ))
    log ""
    log "RESULT: PASS after ${STEP} steps, build took ${BUILD_ELAPSED}s, total ${TOTAL_ELAPSED}s."
    rm -rf "${WORK_DIR}"
    exit 0
else
    fail "cargo build failed against the published crates.io artifact -- see compiler errors above"
fi
