#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
#
# Nightly all-oracle job (CI-006, CI-009; WP17). Runs on the local machine (maintainer decision
# 2026-10-14: nightly and long tiers run locally; GitHub runners do PR-fast work), e.g. from cron,
# as TWO separate jobs that exchange files only (LIC-005; PR #13 judge review LIC-3):
#   17 2 * * *  cd /path/to/FerroAstro && tools/oracles/nightly.sh --open-issue >> target/nightly.log 2>&1
#   17 6 * * *  cd /path/to/FerroAstro && tools/oracles/nightly_ferroastro.sh >> target/nightly-fa.log 2>&1
#
#   tools/oracles/nightly.sh [--open-issue]
#
# This script runs the GPL oracle containers and the drift check only. It never builds, installs
# or runs FerroAstro (no cargo): the FerroAstro side is tools/oracles/nightly_ferroastro.sh, a
# separate job that reads the plain files this one leaves in $FA_ORACLE_OUT_DIR.
#
# 1. Every N-body oracle (rebound, swiftest, mercury6, orekit) on every ci/scenarios/*.toml: a scenario a
#    runner does not implement is refused (exit 3, REF-011) and recorded as such; plus the erfa
#    and jpl images. Outputs: $FA_ORACLE_OUT_DIR/<tool>/<scenario>/ (default target/oracle-out),
#    plain files with MANIFEST.json. Images are built locally and never pushed (LIC-002); no
#    FerroAstro build enters them (run_oracles.sh stages the contexts).
# 2. Drift (tools/oracles/drift.py): every committed ci/golden/<tool>/<name>/ and the ERFA goldens
#    must be reproduced bit for bit. Drift writes a report and opens a BLOCKING issue
#    (labels golden-drift, blocking) when --open-issue is given; without it, a dry run.
# 3. Speed-table stub: in the report (wall clock per tool and scenario, from the manifests).
# 4. Marker $FA_ORACLE_OUT_DIR/oracle-job.done (UTC time and exit status) for the FerroAstro job.
#
# Exit status: 0 when every step passed and nothing drifted; 1 otherwise.
set -uo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$root"
out="${FA_ORACLE_OUT_DIR:-$root/target/oracle-out}"
export FA_ORACLE_OUT_DIR="$out"
open_issue=""
for a in "$@"; do
  case "$a" in
    --open-issue) open_issue="--open-issue" ;;
    --skip-rust) echo "--skip-rust is the default now: the FerroAstro side is nightly_ferroastro.sh (LIC-005)" >&2 ;;
    *) echo "unknown argument $a" >&2; exit 2 ;;
  esac
done
mkdir -p "$out"
status="$out/status.tsv"
: > "$status"
rm -f "$out/oracle-job.done"
fail=0
R=tools/oracles/run_oracles.sh

for tool in rebound swiftest mercury6 orekit erfa jpl; do
  $R build "$tool" > "$out/build-$tool.log" 2>&1 || { echo -e "$tool\t(build)\t1\tbuild failed" >> "$status"; fail=1; }
done

for tool in rebound swiftest mercury6 orekit; do
  for s in ci/scenarios/*.toml; do
    name="$(basename "$s" .toml)"
    # bench-* scenarios belong to the speed-table harness (tools/bench/speed_table.sh, WP18).
    [[ "$name" == bench-* ]] && continue
    rm -rf "${out:?}/$tool/$name"
    log="$out/$tool-$name.log"
    $R "$tool" "$s" "$out/$tool/$name" > "$log" 2>&1
    code=$?
    note="$(grep -E '^(REFUSED|SETUP ERROR|wrote)' "$log" | head -n1 | cut -c1-160 | tr '\t|' '  ')"
    echo -e "$tool\t$name\t$code\t$note" >> "$status"
    if [[ $code == 3 ]]; then rm -rf "${out:?}/$tool/$name"; fi
    if [[ $code != 0 && $code != 3 ]]; then fail=1; fi
  done
done
$R erfa "$out/erfa/erfa-goldens" > "$out/erfa.log" 2>&1; code=$?
echo -e "erfa\terfa-goldens\t$code\t" >> "$status"; [[ $code == 0 ]] || fail=1
$R jpl "$out/jpl/ref001-pins" > "$out/jpl.log" 2>&1; code=$?
echo -e "jpl\tref001-pins\t$code\t" >> "$status"; [[ $code == 0 ]] || fail=1

(cd tools && uv run python -m oracles.drift --candidate "$out" --status "$status" \
  --report "$out/report.md" $open_issue) || fail=1

echo -e "$(date -u +%Y-%m-%dT%H:%M:%SZ)\t$fail" > "$out/oracle-job.done"
echo "nightly oracle job: $([[ $fail == 0 ]] && echo PASS || echo FAIL); report $out/report.md"
echo "the FerroAstro side is a separate job: tools/oracles/nightly_ferroastro.sh (LIC-005)"
exit "$fail"
