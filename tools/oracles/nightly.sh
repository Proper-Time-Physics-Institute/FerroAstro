#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
#
# Nightly all-oracle job (CI-006, CI-009; WP17). Runs on the local machine (maintainer decision
# 2026-10-14: nightly and long tiers run locally; GitHub runners do PR-fast work), e.g. from cron:
#   17 2 * * *  cd /path/to/FerroAstro && tools/oracles/nightly.sh --open-issue >> target/nightly.log 2>&1
#
#   tools/oracles/nightly.sh [--open-issue] [--skip-rust]
#
# 1. Every N-body oracle (rebound, swiftest, mercury6, orekit) on every ci/scenarios/*.toml: a scenario a
#    runner does not implement is refused (exit 3, REF-011) and recorded as such; plus the erfa
#    and jpl images. Outputs: $FA_ORACLE_OUT_DIR/<tool>/<scenario>/ (default target/oracle-out),
#    plain files with MANIFEST.json. Images are built locally and never pushed (LIC-002); no
#    FerroAstro build enters them (run_oracles.sh stages the contexts).
# 2. Drift (tools/oracles/drift.py): every committed ci/golden/<tool>/<name>/ and the ERFA goldens
#    must be reproduced bit for bit. Drift writes a report and opens a BLOCKING issue
#    (labels golden-drift, blocking) when --open-issue is given; without it, a dry run.
# 3. The FerroAstro side (separate process, no container): the nightly oracle differentials
#    (one Park2017 estimator on every tool's series, CI-009) read the outputs of step 1.
# 4. Speed-table stub: in the report (wall clock per tool and scenario, from the manifests).
#
# Run status (progress review after WP18, R3): $out/run-status.txt holds `state=RUNNING` from the
# start, the step in progress, and the final `state=PASS|FAIL|INTERRUPTED|ABORTED` with the wall
# clock. A run killed by a signal (e.g. the controlling session ends: SIGHUP/SIGTERM) records
# INTERRUPTED with the signal and the step in both run-status.txt and status.tsv instead of ending
# silently; any other premature exit records ABORTED. Every step's wall clock is in status.tsv.
#
# Exit status: 0 when every step passed and nothing drifted; 1 otherwise; 128+n when interrupted
# by signal n.
set -uo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$root"
out="${FA_ORACLE_OUT_DIR:-$root/target/oracle-out}"
export FA_ORACLE_OUT_DIR="$out"
open_issue=""
skip_rust=0
for a in "$@"; do
  case "$a" in
    --open-issue) open_issue="--open-issue" ;;
    --skip-rust) skip_rust=1 ;;
    *) echo "unknown argument $a" >&2; exit 2 ;;
  esac
done
mkdir -p "$out"
status="$out/status.tsv"
run_status="$out/run-status.txt"
: > "$status"
fail=0
R=tools/oracles/run_oracles.sh
t_start=$SECONDS
export FA_ORACLE_RUN_ID="nightly-$$-$(date -u +%s)"
current="(start)"
child=""
finished=0
{
  echo "state=RUNNING"
  echo "started_utc=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "commit=$(git rev-parse HEAD 2>/dev/null)$(git diff --quiet 2>/dev/null || echo -dirty)"
  echo "pid=$$"
} > "$run_status"

finish() { # state
  finished=1
  {
    echo "state=$1"
    echo "ended_utc=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "wall_s=$((SECONDS - t_start))"
  } >> "$run_status"
}
on_signal() { # signal-name number
  [[ -n "$child" ]] && kill -TERM "$child" 2>/dev/null
  # Containers outlive their docker client; stop the ones this run started.
  docker ps -q --filter "label=fa.oracle.run=$FA_ORACLE_RUN_ID" | xargs -r docker kill > /dev/null 2>&1
  echo -e "${current%%|*}\t${current#*|}\t$((128 + $2))\tINTERRUPTED by $1 after $((SECONDS - t_start)) s" >> "$status"
  echo "interrupted_by=$1 during=$current" >> "$run_status"
  finish INTERRUPTED
  exit $((128 + $2))
}
on_exit() {
  if [[ $finished == 0 ]]; then
    echo "aborted_during=$current" >> "$run_status"
    finish ABORTED
  fi
}
trap 'on_signal SIGHUP 1' HUP
trap 'on_signal SIGINT 2' INT
trap 'on_signal SIGTERM 15' TERM
trap on_exit EXIT

# Run a command as a background child and wait for it, so that a signal reaches the trap at once.
run_step() { # tool|scenario, logfile, command...
  current="$1"
  echo "step=$1 at=$((SECONDS - t_start))s" >> "$run_status"
  local log="$2"
  shift 2
  "$@" > "$log" 2>&1 &
  child=$!
  wait "$child"
  local code=$?
  child=""
  return "$code"
}

for tool in rebound swiftest mercury6 orekit erfa jpl; do
  t0=$SECONDS
  run_step "$tool|(build)" "$out/build-$tool.log" $R build "$tool" \
    || { echo -e "$tool\t(build)\t1\tbuild failed [$((SECONDS - t0)) s]" >> "$status"; fail=1; }
done

for tool in rebound swiftest mercury6 orekit; do
  for s in ci/scenarios/*.toml; do
    name="$(basename "$s" .toml)"
    # bench-* scenarios belong to the speed-table harness (tools/bench/speed_table.sh, WP18).
    [[ "$name" == bench-* ]] && continue
    rm -rf "${out:?}/$tool/$name"
    log="$out/$tool-$name.log"
    t0=$SECONDS
    run_step "$tool|$name" "$log" $R "$tool" "$s" "$out/$tool/$name"
    code=$?
    note="$(grep -E '^(REFUSED|SETUP ERROR|wrote)' "$log" | head -n1 | cut -c1-160 | tr '\t|' '  ')"
    echo -e "$tool\t$name\t$code\t[$((SECONDS - t0)) s] $note" >> "$status"
    if [[ $code == 3 ]]; then rm -rf "${out:?}/$tool/$name"; fi
    if [[ $code != 0 && $code != 3 ]]; then fail=1; fi
  done
done
t0=$SECONDS
run_step "erfa|erfa-goldens" "$out/erfa.log" $R erfa "$out/erfa/erfa-goldens"; code=$?
echo -e "erfa\terfa-goldens\t$code\t[$((SECONDS - t0)) s]" >> "$status"; [[ $code == 0 ]] || fail=1
t0=$SECONDS
run_step "jpl|ref001-pins" "$out/jpl.log" $R jpl "$out/jpl/ref001-pins"; code=$?
echo -e "jpl\tref001-pins\t$code\t[$((SECONDS - t0)) s]" >> "$status"; [[ $code == 0 ]] || fail=1

current="drift|(drift check)"
echo "step=drift at=$((SECONDS - t_start))s" >> "$run_status"
(cd tools && uv run python -m oracles.drift --candidate "$out" --status "$status" \
  --report "$out/report.md" $open_issue) || { fail=1; echo "drift=FAIL" >> "$run_status"; }

if [[ $skip_rust == 0 ]]; then
  # The FerroAstro side runs here, on the host, after the containers have exited.
  t0=$SECONDS
  run_step "ferroastro|nightly differentials" "$out/ferroastro-nightly.log" \
    cargo nextest run -p ferroastro-oracle --cargo-profile release --profile nightly \
    --run-ignored only --no-capture
  code=$?
  echo -e "ferroastro\tnightly-differentials\t$code\t[$((SECONDS - t0)) s]" >> "$status"
  [[ $code == 0 ]] || fail=1
  grep -E 'P-add Venus|GR contribution|PRECONDITION|PASS|FAIL' "$out/ferroastro-nightly.log" | tail -n 40
fi
result="$([[ $fail == 0 ]] && echo PASS || echo FAIL)"
finish "$result"
echo "nightly oracle job: $result; report $out/report.md; run status $run_status"
exit "$fail"
