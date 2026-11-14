#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
#
# WP18 speed-table harness (CI-008, DEP-014, NUM-009). One invocation = one "workflow run": every
# tool on every bench scenario, >= 5 timed runs each, on this one machine (CI-008: the
# maintainer's local machine, Journal #10). Outputs under $FA_BENCH_DIR/<run-id>/ (default
# target/bench): run.json, <tool>/<scenario>/run-<k>/ and speed-table.{json,md}.
#
#   tools/bench/speed_table.sh [--runs N] [--run-id ID] [--only-ferroastro] [--only SCENARIO]...
#                              [--suite wp18|wp28] [--pin CPU]
#
# --suite wp28 (NUM-024, WP28) times rows (f)-(j): EIH Sun + planets over 100 yr with IAS15, ABM13
# and REBOUNDx gr_full; the NUM-024 f64 and mixed 40-yr runs; the DE440-class mixed run. Their full
# double-double twins (the round-off column) run first, once, untimed and in parallel on other
# cores. --pin CPU (default 6 for wp28, none for wp18) runs every timed FerroAstro run under
# `taskset -c CPU`, and a single-core reference score (`7z b -mmt1` on the same CPU) is recorded at
# the start and the end (OI-44: for a later hardware normalisation; none is applied).
#
# --only (WP21) restricts the invocation to the named timed scenarios (repeatable; their daily
# twins follow); the table then lists only those scenarios and notes the others as not run.
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
only=()
suite=wp18
pin=""
pin_set=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --runs) runs="$2"; shift 2 ;;
    --run-id) run_id="$2"; shift 2 ;;
    --only-ferroastro) only_fa=1; shift ;;
    --only) only+=("$2"); shift 2 ;;
    --suite) suite="$2"; shift 2 ;;
    --pin) pin="$2"; pin_set=1; shift 2 ;;
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

truths=()
case "$suite" in
  wp18)
    timed=(bench-a-sun-8planets-newton-1000yr bench-b-sun-mercury-tp-gr1pn-1000yr
           bench-c-full-newton-minor343-100yr oracle-stage-b-mercury-de440-field-100yr
           bench-e-sun-8planets-eih-1000yr)
    daily=(bench-a-sun-8planets-newton-1000yr-daily bench-b-sun-mercury-tp-gr1pn-1000yr-daily) ;;
  wp28)
    timed=(bench-f-sun-8planets-eih-100yr bench-g-sun-8planets-eih-100yr-abm
           bench-h-num024-epm16-f64-40yr ephemeris-mixed-epm16-40yr bench-j-de440class-mixed-40yr)
    daily=()
    truths=(bench-f-sun-8planets-eih-100yr-dd bench-g-sun-8planets-eih-100yr-abm-dd
            bench-h-num024-epm16-dd-40yr bench-j-de440class-dd-40yr)
    [[ $pin_set == 1 ]] || pin=6 ;;
  *) echo "unknown suite $suite" >&2; exit 2 ;;
esac
if [[ ${#only[@]} -gt 0 ]]; then
  keep_t=(); keep_d=()
  for s in "${only[@]}"; do
    [[ " ${timed[*]} " == *" $s "* ]] || { echo "--only: unknown timed scenario $s" >&2; exit 2; }
    keep_t+=("$s")
    [[ " ${daily[*]} " == *" $s-daily "* ]] && keep_d+=("$s-daily")
  done
  timed=("${keep_t[@]}"); daily=("${keep_d[@]}")
fi

cargo build --release -p ferroastro-bench --example speed_table >&2 || exit 1
drv="$root/target/release/examples/speed_table"
pinned=()
truth_cpus=""
if [[ -n "$pin" ]]; then
  pinned=(taskset -c "$pin")
  # Truth runs: the other P-core threads (this machine: CPUs 0-15 are P-core threads, sibling
  # pairs 2k/2k+1), never the timed CPU or its SMT sibling.
  sib=$(( pin ^ 1 ))
  truth_cpus="$(seq 0 15 | grep -vx -e "$pin" -e "$sib" | paste -sd, -)"
fi
score() { # single-core reference score (7-Zip LZMA total rating, MIPS) on the timed CPU
  command -v 7z > /dev/null || { echo null; return; }
  "${pinned[@]}" 7z b -mmt1 2> /dev/null | awk '/^Tot:/ {print $NF}' | tail -n1
}
score_version="$(command -v 7z > /dev/null && 7z 2> /dev/null | grep -m1 '^7-Zip' || echo none)"
score_start=""
[[ "$suite" == wp28 ]] && score_start="$(score)"

# WP28: the double-double truth runs, once, untimed, in parallel, before any timed run.
if [[ ${#truths[@]} -gt 0 ]]; then
  pids=()
  for s in "${truths[@]}"; do
    rm -rf "$out/ferroastro/$s"
    if [[ -n "$truth_cpus" ]]; then
      taskset -c "$truth_cpus" "$drv" ferroastro --scenario "ci/scenarios/$s.toml" --runs 1 --run-dir "$out" > "$out/truth-$s.log" 2>&1 &
    else
      "$drv" ferroastro --scenario "ci/scenarios/$s.toml" --runs 1 --run-dir "$out" > "$out/truth-$s.log" 2>&1 &
    fi
    pids+=($!)
  done
  for p in "${pids[@]}"; do wait "$p" || { echo "a truth run failed (see $out/truth-*.log)" >&2; exit 1; }; done
  cat "$out"/truth-*.log >&2
fi

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
      # Keep the runner's message before the run directories (and their logs) are removed.
      grep -E '^REFUSED' "$log" | head -n 3 > "$out/$tool/$s/refused.txt"
      [[ -s "$out/$tool/$s/refused.txt" ]] || tail -n 3 "$log" > "$out/$tool/$s/refused.txt"
      mv "$log" "$out/$tool/$s/refused.log"
      rm -rf "${out:?}/$tool/$s"/run-*
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
  "${pinned[@]}" "$drv" ferroastro --scenario "ci/scenarios/$s.toml" --runs "$runs" --run-dir "$out" >&2 || exit 1
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

score_json="null"
if [[ "$suite" == wp28 ]]; then
  score_end="$(score)"
  score_json="{\"command\": \"${pinned[*]:-} 7z b -mmt1\", \"version\": \"$score_version\", \"ratings\": [${score_start:-null}, ${score_end:-null}]}"
fi
cat > "$out/run.json" <<JSON
{"run_id": "$run_id", "runner_class": "local-workstation (maintainer's machine, CI-008 / Journal #10)",
 "git_commit": "$commit", "started_utc": "$started", "finished_utc": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
 "load_start": $load_start, "load_end": $(load), "runs": $runs, "suite": "$suite", "pin": "$pin",
 "reference_score": $score_json}
JSON
"$drv" assemble --run-dir "$out" --out-json "$out/speed-table.json" --out-md "$out/speed-table.md" > "$out/assemble.log" 2>&1 || { tail -n 20 "$out/assemble.log" >&2; exit 1; }
echo "speed table: $out/speed-table.md" >&2
