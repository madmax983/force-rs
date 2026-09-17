#!/usr/bin/env bash
# Onramp snippet CI: flags any `force = "X"` dependency pin in the first-run
# docs (README.md, docs/adr/004-feature-gates.md, docs/guide/01-getting-started.md)
# that does not match the workspace's own version. Network-free, deterministic.
#
# This is the check that should have caught README.md pinning `force = "0.1"`
# through three subsequent releases (0.2, 0.3, 0.4): a newcomer who copies the
# Installation section verbatim gets whatever 0.1.x is on crates.io, not the
# crate this repository actually ships. docs/guide/01-getting-started.md was
# added to the scan after the same drift was found there too (pinned "0.3").
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
python3 "${REPO_ROOT}/scripts/onramp/onramp_snippets.py" check-version-pins
