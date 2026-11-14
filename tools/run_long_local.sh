#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
#
# Run the long tiers on the local machine (maintainer decision 2026-10-15: long/nightly tiers run
# locally; GitHub-hosted runners do PR-fast work). Runs, in order:
#
#   0. `tools/fetch_jpl.py` (the PR-fast set; every manifest product when the data tier runs), so
#      that the nightly tier never meets an empty cache (review F3 after WP10); its output, with
#      each product's SHA-256 and URL, goes into the log;
#   1. the `nightly` nextest profile (nightly_* tests) in release mode;
#   2. the `data` nextest profile (data_* tests: real JPL products, network);
#
# After the nightly tier, `tools/nightly_summary.py` labels each failing test against
# `ci/nightly-known-failures.toml` (known unchanged / known changed / NEW; WP28, review W3).
#
# and writes one log per invocation to target/long-local/<UTC timestamp>-<short sha>.log with the
# commit, a dirty-tree flag, the UTC date, the toolchain (rustc -Vv, cargo -V, nextest -V), the
# data-cache directory and the full test output (the data tests print URL, retrieval date, SHA-256
# and the Horizons signature.version of every product they use).
#
# Usage: tools/run_long_local.sh [--nightly-only | --data-only | --weekly-bench]
#
# --weekly-bench (WP28, NUM-024, CI-008) runs only the speed-table harness's wp28 suite
# (`tools/bench/speed_table.sh --suite wp28`: median of 5 per row, pinned to one P-core, ~1.5 h)
# and then `nightly_num024_speed_table_is_complete` on the table it wrote. Run it on a quiet
# machine; the resulting table is published by copying it to docs/validation/speed-tables/.
# Environment: FA_DATA_DIR (default: target/fa-data). Never set FA_VECTORS_DIR here: the tiers
# must run against the committed vectors (the tests refuse an override; see ferroastro-vectors).
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

run_nightly=1
run_data=1
run_bench=0
case "${1:-}" in
  "") ;;
  --nightly-only) run_data=0 ;;
  --data-only) run_nightly=0 ;;
  --weekly-bench) run_nightly=0; run_data=0; run_bench=1 ;;
  *) echo "usage: $0 [--nightly-only | --data-only | --weekly-bench]" >&2; exit 2 ;;
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
cache_dir="${FA_DATA_DIR:-${CARGO_TARGET_DIR:-$root/target}/fa-data}"
fetch_args=()
if [[ "$run_data" == 1 ]]; then fetch_args=(--all); fi
echo "## fetch_jpl.py ${fetch_args[*]:-(PR-fast set)} (pinned SHA-256; cache hit = no download)" | tee -a "$log"
if ! (cd tools && FA_DATA_DIR="$cache_dir" uv run python fetch_jpl.py "${fetch_args[@]}") 2>&1 | tee -a "$log"; then
  echo "fetch_jpl.py failed; the tiers below will report FA-E0204 for missing products" | tee -a "$log"
  status=1
fi
for side in "$cache_dir"/*.provenance.toml; do
  [[ -e "$side" ]] || continue
  echo "provenance $(basename "$side" .provenance.toml): $(tr '\n' ' ' < "$side")" | tee -a "$log"
done
if [[ "$run_nightly" == 1 ]]; then
  echo "## nightly tier (release)" | tee -a "$log"
  if ! cargo nextest run --workspace --cargo-profile release --profile nightly --run-ignored only 2>&1 | tee -a "$log"; then
    status=1
  fi
  # WP28 (review W3): label each nightly failure against ci/nightly-known-failures.toml as known
  # (value unchanged), known (value changed) or NEW. Reporting only: no bound or status changes.
  echo "## nightly summary (ci/nightly-known-failures.toml)" | tee -a "$log"
  (cd tools && uv run python nightly_summary.py "$root/$log") 2>&1 | tee -a "$log" || true
fi
if [[ "$run_data" == 1 ]]; then
  echo "## data tier (network; provenance printed by each test)" | tee -a "$log"
  if ! cargo nextest run --workspace --profile data --run-ignored only 2>&1 | tee -a "$log"; then
    status=1
  fi
fi
if [[ "$run_bench" == 1 ]]; then
  bench_id="wp28-${stamp}"
  echo "## weekly bench: tools/bench/speed_table.sh --suite wp28 --run-id ${bench_id}" | tee -a "$log"
  if ! tools/bench/speed_table.sh --suite wp28 --run-id "$bench_id" 2>&1 | tee -a "$log"; then
    status=1
  fi
  table="${FA_BENCH_DIR:-$root/target/bench}/${bench_id}/speed-table.json"
  if ! FA_BENCH_NUM024_TABLE="$table" cargo nextest run -p ferroastro-bench --profile nightly \
      --run-ignored only -E 'test(nightly_num024_speed_table_is_complete)' 2>&1 | tee -a "$log"; then
    status=1
  fi
fi
echo "exit status: ${status}; log: ${log}" | tee -a "$log"
exit "$status"
