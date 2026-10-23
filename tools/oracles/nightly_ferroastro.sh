#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
#
# FerroAstro side of the nightly oracle comparison (CI-006, CI-009; WP17), a job of its own
# (LIC-005; PR #13 judge review LIC-3). It runs only FerroAstro's nightly oracle differentials
# (one Park2017 estimator on every tool's series) on the host, reading the plain files that the
# separate oracle job, tools/oracles/nightly.sh, left in $FA_ORACLE_OUT_DIR (default
# target/oracle-out). It never builds, starts or reads a container image and runs no GPL oracle
# code: the two jobs exchange files only (scenario in; state vectors, goldens and manifests out).
#
#   tools/oracles/nightly_ferroastro.sh
#
# Refuses (exit 2) when the oracle job has not finished: $FA_ORACLE_OUT_DIR/oracle-job.done missing.
# Exit status: 0 when the FerroAstro nightly oracle tests pass; 1 otherwise.
set -uo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$root"
out="${FA_ORACLE_OUT_DIR:-$root/target/oracle-out}"
export FA_ORACLE_OUT_DIR="$out"
if [[ ! -f "$out/oracle-job.done" ]]; then
  echo "no finished oracle job in $out (oracle-job.done missing): run tools/oracles/nightly.sh first" >&2
  exit 2
fi
echo "oracle job finished: $(cut -f1 "$out/oracle-job.done") (status $(cut -f2 "$out/oracle-job.done"))"
fail=0
cargo nextest run -p ferroastro-oracle --cargo-profile release --profile nightly \
  --run-ignored only --no-capture > "$out/ferroastro-nightly.log" 2>&1 || fail=1
grep -E 'P-add Venus|GR contribution|PASS|FAIL' "$out/ferroastro-nightly.log" | tail -n 40
echo "nightly FerroAstro oracle differentials: $([[ $fail == 0 ]] && echo PASS || echo FAIL); log $out/ferroastro-nightly.log"
exit "$fail"
