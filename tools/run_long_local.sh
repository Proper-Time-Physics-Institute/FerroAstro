#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
#
# Run the long tiers on the local machine (maintainer decision 2026-10-15: long/nightly tiers run
# locally; GitHub-hosted runners do PR-fast work). Runs, in order:
#
#   1. the `nightly` nextest profile (nightly_* tests) in release mode;
#   2. the `data` nextest profile (data_* tests: real JPL products, network);
#
# and writes one log per invocation to target/long-local/<UTC timestamp>-<short sha>.log with the
# commit, a dirty-tree flag, the UTC date, the toolchain (rustc -Vv, cargo -V, nextest -V), the
# data-cache directory and the full test output (the data tests print URL, retrieval date, SHA-256
# and the Horizons signature.version of every product they use).
#
# Usage: tools/run_long_local.sh [--nightly-only | --data-only]
# Environment: FA_DATA_DIR (default: target/fa-data). Never set FA_VECTORS_DIR here: the tiers
# must run against the committed vectors (the tests refuse an override; see ferroastro-vectors).
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

run_nightly=1
run_data=1
case "${1:-}" in
  "") ;;
  --nightly-only) run_data=0 ;;
  --data-only) run_nightly=0 ;;
  *) echo "usage: $0 [--nightly-only | --data-only]" >&2; exit 2 ;;
esac

if [[ -n "${FA_VECTORS_DIR:-}" ]]; then
  echo "refusing to run: FA_VECTORS_DIR is set (${FA_VECTORS_DIR}); long tiers use the committed vectors" >&2
  exit 2
fi

sha="$(git rev-parse HEAD)"
short="$(git rev-parse --short=12 HEAD)"
dirty="clean"
if [[ -n "$(git status --porcelain)" ]]; then dirty="DIRTY (uncommitted changes)"; fi
stamp="$(date -u +%Y%m%dT%H%M%SZ)"
mkdir -p target/long-local
log="target/long-local/${stamp}-${short}.log"

{
  echo "# FerroAstro long tiers, local run"
  echo "commit:      ${sha} (${dirty})"
  echo "branch:      $(git rev-parse --abbrev-ref HEAD)"
  echo "date (UTC):  $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "host:        $(uname -srm), $(nproc) logical CPUs"
  echo "rustc:"; rustc -Vv | sed 's/^/  /'
  echo "cargo:       $(cargo -V)"
  echo "nextest:     $(cargo nextest --version 2>/dev/null | head -n1)"
  echo "FA_DATA_DIR: ${FA_DATA_DIR:-<unset: target/fa-data>}"
  echo
} | tee "$log"

status=0
if [[ "$run_nightly" == 1 ]]; then
  echo "## nightly tier (release)" | tee -a "$log"
  if ! cargo nextest run --workspace --cargo-profile release --profile nightly --run-ignored only 2>&1 | tee -a "$log"; then
    status=1
  fi
fi
if [[ "$run_data" == 1 ]]; then
  echo "## data tier (network; provenance printed by each test)" | tee -a "$log"
  if ! cargo nextest run --workspace --profile data --run-ignored only 2>&1 | tee -a "$log"; then
    status=1
  fi
fi
echo "exit status: ${status}; log: ${log}" | tee -a "$log"
exit "$status"
