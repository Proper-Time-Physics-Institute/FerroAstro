#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
#
# WP18 speed-table harness (CI-008, DEP-014, NUM-009). One invocation = one "workflow run": every
# tool on every bench scenario, >= 5 timed runs each, on this one machine (CI-008: the
# maintainer's local machine, Journal #10). Outputs under $FA_BENCH_DIR/<run-id>/ (default
# target/bench): run.json, <tool>/<scenario>/run-<k>/ and speed-table.{json,md}.
#
#   tools/bench/speed_table.sh [--runs N] [--run-id ID] [--only-ferroastro]
#
# Oracles run through tools/oracles/run_oracles.sh (images built locally, never pushed; no
# FerroAstro build enters them, LIC-002). A runner that refuses a scenario (exit 3, REF-011) is
# run once and recorded as refused.txt. The daily twins (rate columns) run once, untimed.
set -uo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$root"
runs=5
run_id="$(date -u +%Y%m%dT%H%M%SZ)"
only_fa=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --runs) runs="$2"; shift 2 ;;
    --run-id) run_id="$2"; shift 2 ;;
    --only-ferroastro) only_fa=1; shift ;;
    *) echo "unknown argument $1" >&2; exit 2 ;;
  esac
done
base="${FA_BENCH_DIR:-$root/target/bench}"
out="$base/$run_id"
mkdir -p "$out"
load() { cut -d' ' -f1 /proc/loadavg; }
commit="$(git rev-parse HEAD)"
[[ -n "$(git status --porcelain -- crates tools ci Cargo.toml Cargo.lock)" ]] && commit="${commit}-dirty"
export FA_BENCH_GIT_COMMIT="$commit"
load_start="$(load)"
started="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

timed=(bench-a-sun-8planets-newton-1000yr bench-b-sun-mercury-tp-gr1pn-1000yr
       bench-c-full-newton-minor343-100yr oracle-stage-b-mercury-de440-field-100yr)
daily=(bench-a-sun-8planets-newton-1000yr-daily bench-b-sun-mercury-tp-gr1pn-1000yr-daily)

cargo build --release -p ferroastro-bench --example speed_table >&2 || exit 1
drv="$root/target/release/examples/speed_table"

oracle() { # tool scenario n
  local tool="$1" s="$2" n="$3" k dir log code
  for ((k = 1; k <= n; k++)); do
    dir="$out/$tool/$s/run-$k"; rm -rf "$dir"; mkdir -p "$dir"
    log="$out/$tool/$s/run-$k.log"
    local before; before="$(load)"
    tools/oracles/run_oracles.sh "$tool" "ci/scenarios/$s.toml" "$dir" > "$log" 2>&1
    code=$?
    echo "$before $(load)" > "$dir/load.txt"
    if [[ $code == 3 ]]; then
      rm -rf "$out/$tool/$s"/run-*; mkdir -p "$out/$tool/$s"
      grep -E '^REFUSED' "$log" | head -n 3 > "$out/$tool/$s/refused.txt"
      [[ -s "$out/$tool/$s/refused.txt" ]] || tail -n 3 "$log" > "$out/$tool/$s/refused.txt"
      echo "$tool $s: refused" >&2; return 0
    elif [[ $code != 0 ]]; then
      { echo "exit $code"; tail -n 5 "$log"; } > "$out/$tool/$s/failed.txt"
      echo "$tool $s: FAILED (exit $code)" >&2; return 0
    fi
    echo "$tool $s run $k/$n: $(grep -E '^wrote' "$log" | tail -n1 | cut -c1-120)" >&2
  done
}

for s in "${timed[@]}"; do
  rm -rf "$out/ferroastro/$s"
  "$drv" ferroastro --scenario "ci/scenarios/$s.toml" --runs "$runs" --run-dir "$out" >&2 || exit 1
done
for s in "${daily[@]}"; do
  rm -rf "$out/ferroastro/$s"
  "$drv" ferroastro --scenario "ci/scenarios/$s.toml" --runs 1 --run-dir "$out" >&2 || exit 1
done
if [[ $only_fa == 0 ]]; then
  for tool in rebound swiftest mercury6 orekit; do
    tools/oracles/run_oracles.sh build "$tool" > "$out/build-$tool.log" 2>&1 || { echo "build $tool failed" >&2; exit 1; }
    for s in "${timed[@]}"; do oracle "$tool" "$s" "$runs"; done
    for s in "${daily[@]}"; do
      if [[ -e "$out/$tool/${s%-daily}/refused.txt" ]]; then continue; fi
      oracle "$tool" "$s" 1
    done
  done
fi

cat > "$out/run.json" <<JSON
{"run_id": "$run_id", "runner_class": "local-workstation (maintainer's machine, CI-008 / Journal #10)",
 "git_commit": "$commit", "started_utc": "$started", "finished_utc": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
 "load_start": $load_start, "load_end": $(load), "runs": $runs}
JSON
"$drv" assemble --run-dir "$out" --out-json "$out/speed-table.json" --out-md "$out/speed-table.md" > "$out/assemble.log" 2>&1 || { tail -n 20 "$out/assemble.log" >&2; exit 1; }
echo "speed table: $out/speed-table.md" >&2
