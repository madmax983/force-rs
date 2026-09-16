#!/usr/bin/env bash
# Onramp snippet CI: flags any `force = "X"` dependency pin in the docs that
# does not match the workspace's own version. Network-free, deterministic.
#
# This is the check that should have caught README.md pinning `force = "0.1"`
# through three subsequent releases (0.2, 0.3, 0.4): a newcomer who copies the
# Installation section verbatim gets whatever 0.1.x is on crates.io, not the
# crate this repository actually ships.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
python3 "${REPO_ROOT}/scripts/onramp/onramp_snippets.py" check-version-pins
